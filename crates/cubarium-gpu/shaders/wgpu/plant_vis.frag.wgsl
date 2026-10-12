struct VoxelScene {
    geom: vec4<i32>,
    extent: vec4<i32>,
    knobs: vec4<f32>,
    skyC: vec4<f32>,
    skyHorizon: vec4<f32>,
    bedrockC: vec4<f32>,
    rockC: vec4<f32>,
    soilC: vec4<f32>,
    waterDeepC: vec4<f32>,
    waterSurfaceC: vec4<f32>,
    lightC: vec4<f32>,
    hazeC: vec4<f32>,
    shadeA: vec4<f32>,
    shadeB: vec4<f32>,
    roofK: vec4<f32>,
    waterK: vec4<f32>,
    plantA: vec4<f32>,
    plantB: vec4<f32>,
    tex: vec4<i32>,
    lightK: vec4<f32>,
    ambientC: vec4<f32>,
    sunK: vec4<f32>,
    waterL: vec4<f32>,
    clock: vec4<f32>,
    waterM: vec4<f32>,
    volK: vec4<f32>,
    dayK: vec4<f32>,
    dayL: vec4<f32>,
    sunLean: vec4<f32>,
    cloudK: vec4<f32>,
    cloudBody: vec4<f32>,
    cloudLit: vec4<f32>,
    bandA: vec4<f32>,
    bandB: vec4<f32>,
    bandC: vec4<f32>,
    tint: vec4<f32>,
    dayA: vec4<f32>,
    dayB: vec4<f32>,
    dayC: vec4<f32>,
    cloudDay: vec4<f32>,
    cloudShade: vec4<f32>,
    rainK: vec4<f32>,
    rainC: vec4<f32>,
    fogK: vec4<f32>,
    fogC: vec4<f32>,
    flashC: vec4<f32>,
    strikeK: vec4<f32>,
    strikeL: vec4<f32>,
    poseK: array<vec4<f32>, 64>,
    emitK: array<vec4<f32>, 64>,
    view: vec4<i32>,
    hiMin: vec4<i32>,
    hiMax: vec4<i32>,
    hiK: vec4<f32>,
    hiCells: array<vec4<u32>, 64>,
    figSun: vec4<f32>,
    figMap: vec4<f32>,
    fogM: vec4<f32>,
    rainH: vec4<f32>,
    camera: vec4<i32>,
}

struct EmitCells {
    emitCells: array<vec4<u32>>,
}

struct PlantPush {
    geom: vec4<i32>,
    k: vec4<i32>,
    t: vec4<f32>,
    layer: vec4<f32>,
}

struct PlantCells {
    cells: array<vec4<u32>>,
}

struct PlantTips {
    tips: array<f32>,
}

struct FragmentOutput {
    @location(0) outId: u32,
}

const TRUNK: i32 = 1i;
const CROWN: i32 = 2i;
const CROWN_HEART: i32 = 3i;
const SPROUT: i32 = 4i;
const ANIMAL_INTERIM: i32 = 5i;
const LOG: i32 = 6i;
const FLOOR_MARK: i32 = 7i;
const TEX_TURF_SIDE: i32 = 6i;
const ROLE_BARK: i32 = 1i;
const ROLE_DRAPE: i32 = 3i;
const TEX_BARK_SIDE: i32 = 7i;
const VINE_FLUSH: i32 = 0i;
const VINE_CURTAIN: i32 = 1i;
const VINE_SLIVER_L: i32 = 2i;
const VINE_SLIVER_R: i32 = 3i;
const EMIT_VINES: u32 = 1048576u;
const EMIT_PLANTS: u32 = 2097152u;
const TONE_EMIT: i32 = 6i;
const SMOOTH_SUN: f32 = 0.33333334f;
const SUN_MARCH: i32 = 128i;
const FAR: f32 = 65504f;
const ENTRY_FLAT: i32 = 33554432i;
const ENTRY_FALLING: i32 = 67108864i;
const WALL: f32 = -1000000f;
const FLOW_FULL: f32 = 0.5f;
const FLOW_STEPS: f32 = 3f;
const FLOW_CYCLE: f32 = 64f;
const FLOW_ADVECT: f32 = 0.083333336f;
const LINE_SEG: f32 = 2f;
const LINE_DENSITY: f32 = 0.16f;
const LINE_LIFE_MIN: f32 = 0.3f;
const LINE_LIFE_MAX: f32 = 0.6f;
const BREEZE_MIN: f32 = 0.1f;
const BREEZE_MAX: f32 = 0.3f;
const STAR_CELL: f32 = 1f;
const STAR_SHARE: f32 = 0.16f;
const STAR_GAIN: f32 = 0.9f;
const STAR_MIN: f32 = 0.09f;
const STAR_MAX: f32 = 3f;
const STAR_POWER: f32 = 1.5f;
const STAR_BRIGHT: f32 = 0.9f;
const CLOUD_FREQ: f32 = 0.027777778f;
const CLOUD_FLAT: f32 = 3f;
const CLOUD_WARP: f32 = 20f;
const DAY_HORIZON_T: f32 = 0.55f;
const FOAM_WIDTH: f32 = 0.4f;
const FOAM_BREATHE: f32 = 0.45f;
const FOAM_STEPS: f32 = 8f;
const PLUNGE_RADIUS: f32 = 1.4f;
const PLUNGE_STEPS: f32 = 4f;
const RAIN_CELL: f32 = 2f;
const RAIN_SLOT: f32 = 8f;
const RING_RADIUS: f32 = 0.6f;
const RING_LIFE: f32 = 0.7f;
const FOG_EVOLVE: f32 = 0.07111111f;
const FOG_FREQ: f32 = 0.11111111f;
const FOG_BANK_FREQ: f32 = 0.03125f;
const FOG_BILLOW: f32 = 3f;
const RAIN_DEPTH: vec3<f32> = vec3<f32>(0.1f, 0.32f, 0.66f);
const RAIN_LAYER_SPEED: vec3<f32> = vec3<f32>(1f, 0.82f, 0.66f);
const RAIN_LAYER_ALPHA: vec3<f32> = vec3<f32>(1f, 0.72f, 0.48f);
const RAIN_SPEED: vec3<f32> = vec3<f32>(5f, 24f, 34f);
const RAIN_LEN: vec3<f32> = vec3<f32>(0.2f, 1.1f, 2.4f);
const RAIN_LANE: vec3<f32> = vec3<f32>(0.55f, 0.7f, 0.5f);
const RAIN_FILL: vec3<f32> = vec3<f32>(0.28f, 0.3f, 0.55f);
const RAIN_ALPHA: vec3<f32> = vec3<f32>(0.4f, 0.3f, 0.32f);
const RAIN_SLANT: vec3<f32> = vec3<f32>(0.04f, 0.1f, 0.24f);
const RAIN_VEIL: vec3<f32> = vec3<f32>(0.1f, 0.05f, 0.16f);
const RAIN_CURTAIN: f32 = 0.45f;
const RAIN_CURTAIN_RAMP: f32 = 0.25f;
const RAIN_CURTAIN_ALPHA: f32 = 1.8f;
const GUST_LOW: f32 = 0.35f;
const SPLASH_SLOT: f32 = 0.5f;
const SPLASH_LIFE: f32 = 0.36f;
const SPLASH_CHANCE: vec3<f32> = vec3<f32>(0.08f, 0.22f, 0.4f);
const BOLT_SEGS: i32 = 18i;
const FORK_SEGS: i32 = 5i;
const BOLT_SWING: f32 = 0.5f;
const BOLT_KINK: f32 = 0.18f;
const VOL_SAMPLES: i32 = 24i;

@id(0) override LIT: bool = false;
@id(1) override SHADOWS: bool = true;
@id(2) override REFLECTIONS: bool = true;
@id(3) override VOLUMETRIC: bool = false;
@id(4) override GLOW: bool = true;
@id(5) override AO: bool = true;
@id(6) override SKY: bool = false;
@id(7) override WEATHER: bool = false;
@id(8) override WEATHER_DEPTH: bool = false;
@id(9) override POSE: bool = false;
@id(10) override PLANT_MODELS: bool = false;
@id(11) override PLANT_LIGHT: bool = true;

@group(0) @binding(32) 
var wgpu_sampler_0_32_: sampler;
@group(0) @binding(33) 
var wgpu_sampler_0_33_: sampler;
@group(2) @binding(32) 
var wgpu_sampler_2_32_: sampler;
@group(0) @binding(0) 
var<uniform> u: VoxelScene;
@group(0) @binding(1) 
var wgpu_voxels: texture_3d<u32>;
@group(0) @binding(2) 
var wgpu_roofTex: texture_3d<u32>;
@group(0) @binding(3) 
var wgpu_styleTex: texture_2d<f32>;
@group(0) @binding(4) 
var wgpu_glyphTex: texture_2d<u32>;
@group(0) @binding(5) 
var wgpu_faceTex: texture_2d<f32>;
@group(0) @binding(6) 
var wgpu_vineTex: texture_2d<f32>;
@group(0) @binding(7) 
var wgpu_skyTex: texture_3d<u32>;
@group(0) @binding(8) 
var wgpu_canopyTex: texture_2d<u32>;
@group(0) @binding(9) 
var wgpu_glowTex: texture_3d<f32>;
@group(0) @binding(11) 
var wgpu_backdropTex: texture_2d<f32>;
@group(0) @binding(12) 
var wgpu_depthTex: texture_2d<f32>;
@group(0) @binding(13) 
var wgpu_figSunTex: texture_depth_2d;
@group(0) @binding(14) 
var<storage> global: EmitCells;
var<private> S: i32;
var<private> RISE: i32;
var<private> BASE: i32;
var<private> W: i32;
var<private> H: i32;
var<private> D: i32;
var<immediate> pc: PlantPush;
@group(1) @binding(0) 
var<storage> global_1: PlantCells;
@group(1) @binding(1) 
var<storage> global_2: PlantTips;
var<private> outId: u32;
var<private> wEntry: i32;
var<private> wScatter: vec3<f32>;
var<private> wHaze: f32;
var<private> wSurf: f32;
var<private> PX: i32;
var<private> PY: i32;
@group(0) @binding(10) 
var wgpu_sunVisTex: texture_3d<f32>;
var<private> fogPixelNoise: f32 = 1f;
var<private> vId_1: u32;
var<private> vUV_1: vec2<f32>;
var<private> gl_FragCoord_1: vec4<f32>;

fn wrapMod(a: i32, m: i32) -> i32 {
    var a_1: i32;
    var m_1: i32;
    var local: i32;

    a_1 = a;
    m_1 = m;
    let _e167: i32 = a_1;
    if (_e167 >= 0i) {
        let _e170: i32 = a_1;
        let _e171: i32 = m_1;
        local = (_e170 % _e171);
    } else {
        let _e173: i32 = m_1;
        let _e178: i32 = a_1;
        let _e180: i32 = m_1;
        local = ((_e173 - 1i) - ((-1i - _e178) % _e180));
    }
    let _e184: i32 = local;
    return _e184;
}

fn wrapX(x: i32) -> i32 {
    var x_1: i32;

    x_1 = x;
    let _e165: i32 = x_1;
    let _e166: i32 = W;
    let _e167: i32 = wrapMod(_e165, _e166);
    return _e167;
}

fn inY(y: i32) -> bool {
    var y_1: i32;

    y_1 = y;
    let _e165: i32 = y_1;
    let _e168: i32 = y_1;
    let _e169: i32 = H;
    return ((_e165 >= 0i) && (_e168 < _e169));
}

fn at(x_2: i32, y_2: i32, z: i32) -> vec4<u32> {
    var x_3: i32;
    var y_3: i32;
    var z_1: i32;

    x_3 = x_2;
    y_3 = y_2;
    z_1 = z;
    let _e169: i32 = x_3;
    let _e170: i32 = wrapX(_e169);
    let _e171: i32 = y_3;
    let _e172: i32 = z_1;
    let _e175: vec4<u32> = textureLoad(wgpu_voxels, vec3<i32>(_e170, _e171, _e172), 0i);
    return _e175;
}

fn matOf(v: vec4<u32>) -> i32 {
    var v_1: vec4<u32>;

    v_1 = v;
    let _e165: vec4<u32> = v_1;
    return i32((_e165.x & 3u));
}

fn partOf(v_2: vec4<u32>) -> i32 {
    var v_3: vec4<u32>;

    v_3 = v_2;
    let _e165: vec4<u32> = v_3;
    return i32(((_e165.x >> 2u) & 7u));
}

fn glyphOf(v_4: vec4<u32>) -> i32 {
    var v_5: vec4<u32>;

    v_5 = v_4;
    let _e165: vec4<u32> = v_5;
    return i32((_e165.x >> 5u));
}

fn isBlockPart(p: i32) -> bool {
    var p_1: i32;

    p_1 = p;
    let _e165: i32 = p_1;
    let _e167: i32 = p_1;
    let _e170: i32 = p_1;
    let _e173: i32 = p_1;
    let _e176: i32 = p_1;
    return (((((_e165 == TRUNK) || (_e167 == CROWN)) || (_e170 == CROWN_HEART)) || (_e173 == ANIMAL_INTERIM)) || (_e176 == LOG));
}

fn plantBlockPart(p_2: i32) -> bool {
    var p_3: i32;

    p_3 = p_2;
    let _e165: i32 = p_3;
    let _e167: i32 = p_3;
    let _e170: i32 = p_3;
    return (((_e165 == TRUNK) || (_e167 == CROWN)) || (_e170 == CROWN_HEART));
}

fn softMark(v_6: vec4<u32>) -> bool {
    var v_7: vec4<u32>;

    v_7 = v_6;
    let _e165: vec4<u32> = v_7;
    let _e166: i32 = partOf(_e165);
    let _e169: vec4<u32> = v_7;
    let _e170: i32 = glyphOf(_e169);
    let _e174: vec4<u32> = v_7;
    let _e179: vec4<u32> = v_7;
    return ((((POSE && (_e166 == ANIMAL_INTERIM)) && (_e170 == 0i)) && (_e174.z != 0u)) && ((_e179.z >> 6u) < 2u));
}

fn solidV(v_8: vec4<u32>) -> bool {
    var v_9: vec4<u32>;

    v_9 = v_8;
    let _e165: vec4<u32> = v_9;
    let _e166: i32 = matOf(_e165);
    return (_e166 != 0i);
}

fn solidAt(x_4: i32, y_4: i32, z_2: i32) -> bool {
    var x_5: i32;
    var y_5: i32;
    var z_3: i32;

    x_5 = x_4;
    y_5 = y_4;
    z_3 = z_2;
    let _e169: i32 = y_5;
    let _e170: bool = inY(_e169);
    let _e171: i32 = x_5;
    let _e172: i32 = y_5;
    let _e173: i32 = z_3;
    let _e174: vec4<u32> = at(_e171, _e172, _e173);
    let _e175: bool = solidV(_e174);
    return (_e170 && _e175);
}

fn freeQ(x_6: i32, y_6: i32, z_4: i32) -> i32 {
    var x_7: i32;
    var y_7: i32;
    var z_5: i32;
    var local_1: i32;

    x_7 = x_6;
    y_7 = y_6;
    z_5 = z_4;
    let _e169: i32 = y_7;
    let _e170: bool = inY(_e169);
    if _e170 {
        let _e171: i32 = x_7;
        let _e172: i32 = y_7;
        let _e173: i32 = z_5;
        let _e174: vec4<u32> = at(_e171, _e172, _e173);
        local_1 = i32(_e174.y);
    } else {
        local_1 = 0i;
    }
    let _e179: i32 = local_1;
    return _e179;
}

fn fillPxQ(q: i32) -> i32 {
    var q_1: i32;
    var local_2: i32;

    q_1 = q;
    let _e165: i32 = q_1;
    if (_e165 == 0i) {
        local_2 = 0i;
    } else {
        let _e169: i32 = q_1;
        let _e173: i32 = S;
        let _e181: i32 = S;
        local_2 = clamp(i32(floor((((f32(_e169) / 255f) * f32(_e173)) + 0.5f))), 1i, _e181);
    }
    let _e184: i32 = local_2;
    return _e184;
}

fn fillPxAt(x_8: i32, y_8: i32, z_6: i32) -> i32 {
    var x_9: i32;
    var y_9: i32;
    var z_7: i32;

    x_9 = x_8;
    y_9 = y_8;
    z_7 = z_6;
    let _e169: i32 = x_9;
    let _e170: i32 = y_9;
    let _e171: i32 = z_7;
    let _e172: i32 = freeQ(_e169, _e170, _e171);
    let _e173: i32 = fillPxQ(_e172);
    return _e173;
}

fn waterOpenUp(x_10: i32, y_10: i32, z_8: i32) -> bool {
    var x_11: i32;
    var y_11: i32;
    var z_9: i32;

    x_11 = x_10;
    y_11 = y_10;
    z_9 = z_8;
    let _e169: i32 = x_11;
    let _e170: i32 = y_11;
    let _e171: i32 = z_9;
    let _e172: i32 = fillPxAt(_e169, _e170, _e171);
    let _e173: i32 = S;
    let _e175: i32 = x_11;
    let _e176: i32 = y_11;
    let _e179: i32 = z_9;
    let _e180: bool = solidAt(_e175, (_e176 + 1i), _e179);
    let _e182: i32 = x_11;
    let _e183: i32 = y_11;
    let _e186: i32 = z_9;
    let _e187: i32 = freeQ(_e182, (_e183 + 1i), _e186);
    return ((_e172 < _e173) || (!(_e180) && (_e187 == 0i)));
}

fn frontRow(y_12: i32, z_10: i32) -> i32 {
    var y_13: i32;
    var z_11: i32;

    y_13 = y_12;
    z_11 = z_10;
    let _e167: i32 = BASE;
    let _e168: i32 = y_13;
    let _e171: i32 = S;
    let _e174: i32 = z_11;
    let _e175: i32 = RISE;
    return ((_e167 - ((_e168 + 1i) * _e171)) - (_e174 * _e175));
}

fn texOn(slot: i32) -> bool {
    var slot_1: i32;

    slot_1 = slot;
    let _e169: i32 = slot_1;
    let _e172: VoxelScene = u;
    let _e176: i32 = slot_1;
    return ((_e169 >= 0i) && ((_e172.tex.x & (1i << u32(_e176))) != 0i));
}

fn cellHash(x_12: i32, y_14: i32, z_12: i32, salt: i32) -> u32 {
    var x_13: i32;
    var y_15: i32;
    var z_13: i32;
    var salt_1: i32;
    var h: u32;

    x_13 = x_12;
    y_15 = y_14;
    z_13 = z_12;
    salt_1 = salt;
    let _e175: i32 = x_13;
    let _e179: i32 = y_15;
    let _e184: i32 = z_13;
    let _e189: i32 = salt_1;
    h = ((((u32(_e175) * 2654435761u) ^ (u32(_e179) * 2246822519u)) ^ (u32(_e184) * 3266489917u)) ^ (u32(_e189) * 668265263u));
    let _e195: u32 = h;
    let _e196: u32 = h;
    h = (_e195 ^ (_e196 >> 15u));
    let _e201: u32 = h;
    h = (_e201 * 739982445u);
    let _e204: u32 = h;
    let _e205: u32 = h;
    h = (_e204 ^ (_e205 >> 12u));
    let _e210: u32 = h;
    return _e210;
}

fn texVariant(x_14: i32, y_16: i32, z_14: i32, salt_2: i32) -> i32 {
    var x_15: i32;
    var y_17: i32;
    var z_15: i32;
    var salt_3: i32;

    x_15 = x_14;
    y_17 = y_16;
    z_15 = z_14;
    salt_3 = salt_2;
    let _e175: i32 = x_15;
    let _e176: i32 = y_17;
    let _e177: i32 = z_15;
    let _e178: i32 = salt_3;
    let _e179: u32 = cellHash(_e175, _e176, _e177, _e178);
    return i32((_e179 & 3u));
}

fn faceTexel(slot_2: i32, x_16: i32, y_18: i32, z_16: i32, salt_4: i32, dx: i32, dy: i32) -> vec4<f32> {
    var slot_3: i32;
    var x_17: i32;
    var y_19: i32;
    var z_17: i32;
    var salt_5: i32;
    var dx_1: i32;
    var dy_1: i32;
    var v_10: i32;

    slot_3 = slot_2;
    x_17 = x_16;
    y_19 = y_18;
    z_17 = z_16;
    salt_5 = salt_4;
    dx_1 = dx;
    dy_1 = dy;
    let _e181: i32 = x_17;
    let _e182: i32 = y_19;
    let _e183: i32 = z_17;
    let _e184: i32 = salt_5;
    let _e185: i32 = texVariant(_e181, _e182, _e183, _e184);
    v_10 = _e185;
    let _e187: i32 = v_10;
    let _e188: i32 = S;
    let _e190: i32 = dx_1;
    let _e192: i32 = slot_3;
    let _e193: i32 = S;
    let _e195: i32 = dy_1;
    let _e199: vec4<f32> = textureLoad(wgpu_faceTex, vec2<i32>(((_e187 * _e188) + _e190), ((_e192 * _e193) + _e195)), 0i);
    return _e199;
}

fn srgbToLinear(c: vec3<f32>) -> vec3<f32> {
    var c_1: vec3<f32>;

    c_1 = c;
    let _e169: vec3<f32> = c_1;
    let _e173: vec3<f32> = c_1;
    let _e185: vec3<f32> = c_1;
    return mix((_e169 / vec3(12.92f)), pow(((_e173 + vec3(0.055f)) / vec3(1.055f)), vec3(2.4f)), step(vec3(0.04045f), _e185));
}

fn isVine(v_11: vec4<u32>) -> bool {
    var v_12: vec4<u32>;

    v_12 = v_11;
    let _e173: VoxelScene = u;
    let _e178: vec4<u32> = v_12;
    let _e185: vec4<u32> = v_12;
    return (((_e173.tex.y != 0i) && ((_e178.x & 31u) == 0u)) && ((_e185.x >> 5u) != 0u));
}

fn vineKind(v_13: vec4<u32>) -> i32 {
    var v_14: vec4<u32>;

    v_14 = v_13;
    let _e173: vec4<u32> = v_14;
    return (i32((_e173.x >> 5u)) - 1i);
}

fn vineTexel(v_15: vec4<u32>, x_18: i32, y_20: i32, z_18: i32, dx_2: i32, dy_2: i32) -> vec4<f32> {
    var v_16: vec4<u32>;
    var x_19: i32;
    var y_21: i32;
    var z_19: i32;
    var dx_3: i32;
    var dy_3: i32;
    var row: i32;
    var col: i32;
    var t: vec4<f32>;
    var accent: i32;
    var j: i32;
    var h_1: u32;
    var local_3: i32;
    var ox: i32;
    var local_4: i32;
    var oy: i32;
    var ax: i32;
    var ay: i32;
    var a_2: vec4<f32>;

    v_16 = v_15;
    x_19 = x_18;
    y_21 = y_20;
    z_19 = z_18;
    dx_3 = dx_2;
    dy_3 = dy_2;
    let _e183: vec4<u32> = v_16;
    row = i32((_e183.w & 15u));
    let _e189: vec4<u32> = v_16;
    let _e196: vec4<u32> = v_16;
    col = ((i32((_e189.z & 15u)) * 16i) + i32((_e196.z >> 4u)));
    let _e204: i32 = col;
    let _e205: i32 = S;
    let _e207: i32 = dx_3;
    let _e209: i32 = row;
    let _e210: i32 = S;
    let _e212: i32 = dy_3;
    let _e216: vec4<f32> = textureLoad(wgpu_vineTex, vec2<i32>(((_e204 * _e205) + _e207), ((_e209 * _e210) + _e212)), 0i);
    t = _e216;
    let _e218: vec4<u32> = v_16;
    accent = i32(((_e218.w >> 4u) & 3u));
    let _e227: i32 = accent;
    if (_e227 != 0i) {
        {
            let _e230: i32 = S;
            j = (_e230 / 8i);
            let _e234: i32 = x_19;
            let _e235: i32 = y_21;
            let _e236: i32 = z_19;
            let _e238: u32 = cellHash(_e234, _e235, _e236, 9i);
            h_1 = _e238;
            let _e240: i32 = j;
            if (_e240 > 0i) {
                let _e243: u32 = h_1;
                let _e245: i32 = j;
                let _e252: i32 = j;
                local_3 = (i32((_e243 % u32(((2i * _e245) + 1i)))) - _e252);
            } else {
                local_3 = 0i;
            }
            let _e256: i32 = local_3;
            ox = _e256;
            let _e258: i32 = j;
            if (_e258 > 0i) {
                let _e261: u32 = h_1;
                let _e266: i32 = j;
                let _e273: i32 = j;
                local_4 = (i32(((_e261 >> 8u) % u32(((2i * _e266) + 1i)))) - _e273);
            } else {
                local_4 = 0i;
            }
            let _e277: i32 = local_4;
            oy = _e277;
            let _e279: i32 = dx_3;
            let _e280: i32 = ox;
            ax = (_e279 - _e280);
            let _e283: i32 = dy_3;
            let _e284: i32 = oy;
            ay = (_e283 - _e284);
            let _e287: i32 = ax;
            let _e290: i32 = ax;
            let _e291: i32 = S;
            let _e294: i32 = ay;
            let _e298: i32 = ay;
            let _e299: i32 = S;
            if ((((_e287 >= 0i) && (_e290 < _e291)) && (_e294 >= 0i)) && (_e298 < _e299)) {
                {
                    let _e302: i32 = accent;
                    let _e305: i32 = S;
                    let _e307: i32 = ax;
                    let _e310: i32 = S;
                    let _e312: i32 = ay;
                    let _e316: vec4<f32> = textureLoad(wgpu_vineTex, vec2<i32>((((_e302 - 1i) * _e305) + _e307), ((9i * _e310) + _e312)), 0i);
                    a_2 = _e316;
                    let _e318: vec4<f32> = a_2;
                    if (_e318.w >= 0.5f) {
                        {
                            let _e322: vec4<f32> = a_2;
                            t = _e322;
                        }
                    }
                }
            }
        }
    }
    let _e323: vec4<f32> = t;
    let _e325: vec3<f32> = srgbToLinear(_e323.xyz);
    let _e326: vec4<f32> = t;
    return vec4<f32>(_e325.x, _e325.y, _e325.z, _e326.w);
}

fn glyphTexel(part: i32, glyph: i32, dx_4: i32, row_1: i32) -> u32 {
    var part_1: i32;
    var glyph_1: i32;
    var dx_5: i32;
    var row_2: i32;

    part_1 = part;
    glyph_1 = glyph;
    dx_5 = dx_4;
    row_2 = row_1;
    let _e182: i32 = dx_5;
    let _e183: i32 = part_1;
    let _e186: i32 = glyph_1;
    let _e188: i32 = S;
    let _e189: i32 = RISE;
    let _e192: i32 = row_2;
    let _e196: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e182, ((((_e183 * 8i) + _e186) * (_e188 + _e189)) + _e192)), 0i);
    return _e196.x;
}

fn cordCell(v_17: vec4<u32>) -> bool {
    var v_18: vec4<u32>;

    v_18 = v_17;
    let _e173: VoxelScene = u;
    let _e178: vec4<u32> = v_18;
    let _e179: i32 = partOf(_e178);
    let _e183: vec4<u32> = v_18;
    let _e188: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e183.w)), 0i);
    return (((_e173.tex.z > 0i) && (_e179 == CROWN)) && (i32((_e188.w + 0.5f)) == ROLE_DRAPE));
}

fn cordJoin(x_20: i32, yf: i32, z_20: i32) -> f32 {
    var x_21: i32;
    var yf_1: i32;
    var z_21: i32;
    var w: i32;
    var free: i32;
    var span: i32;
    var lo: i32;
    var h_2: u32;

    x_21 = x_20;
    yf_1 = yf;
    z_21 = z_20;
    let _e177: VoxelScene = u;
    w = _e177.tex.z;
    let _e181: i32 = S;
    let _e182: i32 = w;
    free = max((_e181 - _e182), 0i);
    let _e187: i32 = free;
    let _e189: VoxelScene = u;
    span = i32((((f32(_e187) * f32(((_e189.tex.w >> 8u) & 255i))) / 100f) + 0.5f));
    let _e205: i32 = free;
    let _e206: i32 = span;
    lo = ((_e205 - _e206) / 2i);
    let _e211: i32 = x_21;
    let _e212: i32 = wrapX(_e211);
    let _e213: i32 = yf_1;
    let _e214: i32 = z_21;
    let _e216: u32 = cellHash(_e212, _e213, _e214, 77i);
    h_2 = _e216;
    let _e218: i32 = lo;
    let _e219: u32 = h_2;
    let _e220: i32 = span;
    let _e229: i32 = w;
    return (f32((_e218 + i32((_e219 % u32((_e220 + 1i)))))) + (0.5f * f32(_e229)));
}

fn cordEnds(x_22: i32, y_22: i32, z_22: i32) -> vec2<f32> {
    var x_23: i32;
    var y_23: i32;
    var z_23: i32;

    x_23 = x_22;
    y_23 = y_22;
    z_23 = z_22;
    let _e177: i32 = x_23;
    let _e178: i32 = y_23;
    let _e181: i32 = z_23;
    let _e182: f32 = cordJoin(_e177, (_e178 + 1i), _e181);
    let _e183: i32 = x_23;
    let _e184: i32 = y_23;
    let _e185: i32 = z_23;
    let _e186: f32 = cordJoin(_e183, _e184, _e185);
    return vec2<f32>(_e182, _e186);
}

fn cordAt(a_3: f32, b: f32, t_1: f32) -> f32 {
    var a_4: f32;
    var b_1: f32;
    var t_2: f32;
    var local_5: f32;
    var e: f32;

    a_4 = a_3;
    b_1 = b;
    t_2 = t_1;
    let _e177: VoxelScene = u;
    if ((_e177.tex.w & 1i) == 1i) {
        let _e184: f32 = t_2;
        let _e185: f32 = t_2;
        let _e189: f32 = t_2;
        local_5 = ((_e184 * _e185) * (3f - (2f * _e189)));
    } else {
        let _e193: f32 = t_2;
        local_5 = _e193;
    }
    let _e195: f32 = local_5;
    e = _e195;
    let _e197: f32 = a_4;
    let _e198: f32 = b_1;
    let _e199: f32 = e;
    return mix(_e197, _e198, _e199);
}

fn cordCovers(a_5: f32, b_2: f32, t_3: f32, xp: f32) -> bool {
    var a_6: f32;
    var b_3: f32;
    var t_4: f32;
    var xp_1: f32;
    var w_1: f32;
    var c_2: f32;
    var dt: f32;
    var up: f32;
    var down: f32;
    var ext: f32;
    var r: f32;

    a_6 = a_5;
    b_3 = b_2;
    t_4 = t_3;
    xp_1 = xp;
    let _e179: VoxelScene = u;
    w_1 = f32(_e179.tex.z);
    let _e184: f32 = a_6;
    let _e185: f32 = b_3;
    let _e186: f32 = t_4;
    let _e187: f32 = cordAt(_e184, _e185, _e186);
    c_2 = _e187;
    let _e190: i32 = S;
    dt = (1f / f32(_e190));
    let _e194: f32 = c_2;
    let _e195: f32 = a_6;
    let _e196: f32 = b_3;
    let _e197: f32 = t_4;
    let _e198: f32 = dt;
    let _e202: f32 = cordAt(_e195, _e196, max((_e197 - _e198), 0f));
    up = abs((_e194 - _e202));
    let _e206: f32 = a_6;
    let _e207: f32 = b_3;
    let _e208: f32 = t_4;
    let _e209: f32 = dt;
    let _e213: f32 = cordAt(_e206, _e207, min((_e208 + _e209), 1f));
    let _e214: f32 = c_2;
    down = abs((_e213 - _e214));
    let _e219: f32 = up;
    let _e220: f32 = down;
    ext = max(((0.5f * max(_e219, _e220)) - 0.5f), 0f);
    let _e228: f32 = xp_1;
    let _e229: f32 = c_2;
    r = (_e228 - _e229);
    let _e232: f32 = r;
    let _e235: f32 = w_1;
    let _e237: f32 = ext;
    let _e240: f32 = r;
    let _e242: f32 = w_1;
    let _e244: f32 = ext;
    return ((_e232 >= ((-0.5f * _e235) - _e237)) && (_e240 < ((0.5f * _e242) + _e244)));
}

fn cordPeriod() -> f32 {
    let _e171: VoxelScene = u;
    return (f32(((_e171.tex.w >> 16u) & 255i)) / 4f);
}

fn cordWave() -> bool {
    let _e171: VoxelScene = u;
    return (((_e171.tex.w >> 16u) & 255i) != 0i);
}

fn cordAmp(walk: bool) -> f32 {
    var walk_1: bool;
    var a_7: f32;
    var local_6: f32;

    walk_1 = walk;
    let _e173: VoxelScene = u;
    a_7 = (f32(((_e173.tex.w >> 24u) & 255i)) / 32f);
    let _e185: bool = walk_1;
    if _e185 {
        let _e186: f32 = a_7;
        let _e187: i32 = S;
        let _e188: VoxelScene = u;
        let _e194: i32 = S;
        local_6 = min(_e186, max((f32((_e187 - _e188.tex.z)) / (2f * f32(_e194))), 0f));
    } else {
        let _e201: f32 = a_7;
        local_6 = _e201;
    }
    let _e203: f32 = local_6;
    return _e203;
}

fn cordPhase(x_24: i32, z_24: i32, k: i32) -> f32 {
    var x_25: i32;
    var z_25: i32;
    var k_1: i32;

    x_25 = x_24;
    z_25 = z_24;
    k_1 = k;
    let _e177: i32 = x_25;
    let _e178: i32 = wrapX(_e177);
    let _e179: i32 = k_1;
    let _e180: i32 = z_25;
    let _e182: u32 = cellHash(_e178, _e179, _e180, 78i);
    return (f32((_e182 >> 8u)) / 16777216f);
}

fn cordFreq(x_26: i32, z_26: i32, k_2: i32) -> f32 {
    var x_27: i32;
    var z_27: i32;
    var k_3: i32;
    var o: f32;
    var h_3: f32;

    x_27 = x_26;
    z_27 = z_26;
    k_3 = k_2;
    let _e178: i32 = k_3;
    o = f32((1i << u32(_e178)));
    let _e183: i32 = k_3;
    let _e186: VoxelScene = u;
    if ((_e183 == 0i) || ((_e186.tex.w & 2i) == 0i)) {
        {
            let _e194: f32 = o;
            return _e194;
        }
    }
    let _e195: i32 = x_27;
    let _e196: i32 = wrapX(_e195);
    let _e197: i32 = k_3;
    let _e198: i32 = z_27;
    let _e200: u32 = cellHash(_e196, _e197, _e198, 79i);
    h_3 = (f32((_e200 >> 8u)) / 16777216f);
    let _e207: f32 = o;
    let _e211: f32 = h_3;
    return (_e207 * (1.15f + (0.05f * ((2f * _e211) - 1f))));
}

fn cordOctaves() -> i32 {
    let _e171: VoxelScene = u;
    return max(((_e171.tex.w >> 4u) & 15i), 1i);
}

fn cordWaveAt(x_28: i32, z_28: i32, yw: f32, amp: f32) -> f32 {
    var x_29: i32;
    var z_29: i32;
    var yw_1: f32;
    var amp_1: f32;
    var p_4: f32;
    var sum: f32 = 0f;
    var g: f32 = 1f;
    var gs: f32 = 0f;
    var n: i32;
    var k_4: i32 = 0i;

    x_29 = x_28;
    z_29 = z_28;
    yw_1 = yw;
    amp_1 = amp;
    let _e179: f32 = cordPeriod();
    p_4 = max(_e179, 0.25f);
    let _e189: i32 = cordOctaves();
    n = _e189;
    loop {
        let _e193: i32 = k_4;
        let _e194: i32 = n;
        if !((_e193 < _e194)) {
            break;
        }
        {
            let _e200: f32 = sum;
            let _e201: f32 = g;
            let _e203: f32 = yw_1;
            let _e204: i32 = x_29;
            let _e205: i32 = z_29;
            let _e206: i32 = k_4;
            let _e207: f32 = cordFreq(_e204, _e205, _e206);
            let _e209: f32 = p_4;
            let _e211: i32 = x_29;
            let _e212: i32 = z_29;
            let _e213: i32 = k_4;
            let _e214: f32 = cordPhase(_e211, _e212, _e213);
            sum = (_e200 + (_e201 * sin((6.2831855f * (((_e203 * _e207) / _e209) + _e214)))));
            let _e220: f32 = gs;
            let _e221: f32 = g;
            gs = (_e220 + _e221);
            let _e223: f32 = g;
            g = (_e223 * 0.5f);
        }
        continuing {
            let _e197: i32 = k_4;
            k_4 = (_e197 + 1i);
        }
    }
    let _e226: i32 = S;
    let _e229: f32 = amp_1;
    let _e230: f32 = sum;
    let _e232: f32 = gs;
    return (f32(_e226) * (0.5f + ((_e229 * _e230) / _e232)));
}

fn cordWaveCentre(p_5: vec3<i32>, t_5: f32, amp_2: f32, sh: vec2<f32>) -> f32 {
    var p_6: vec3<i32>;
    var t_6: f32;
    var amp_3: f32;
    var sh_1: vec2<f32>;

    p_6 = p_5;
    t_6 = t_5;
    amp_3 = amp_2;
    sh_1 = sh;
    let _e179: vec3<i32> = p_6;
    let _e181: vec3<i32> = p_6;
    let _e183: vec3<i32> = p_6;
    let _e188: f32 = t_6;
    let _e190: f32 = amp_3;
    let _e191: f32 = cordWaveAt(_e179.x, _e181.z, ((f32(_e183.y) + 1f) - _e188), _e190);
    let _e192: vec2<f32> = sh_1;
    let _e194: vec2<f32> = sh_1;
    let _e196: f32 = t_6;
    return (_e191 + mix(_e192.x, _e194.y, _e196));
}

fn cordSpan(c_3: f32, up_1: f32, down_1: f32, xp_2: f32) -> bool {
    var c_4: f32;
    var up_2: f32;
    var down_2: f32;
    var xp_3: f32;
    var w_2: f32;
    var ext_1: f32;
    var r_1: f32;

    c_4 = c_3;
    up_2 = up_1;
    down_2 = down_1;
    xp_3 = xp_2;
    let _e179: VoxelScene = u;
    w_2 = f32(_e179.tex.z);
    let _e185: f32 = up_2;
    let _e186: f32 = down_2;
    ext_1 = max(((0.5f * max(_e185, _e186)) - 0.5f), 0f);
    let _e194: f32 = xp_3;
    let _e195: f32 = c_4;
    r_1 = (_e194 - _e195);
    let _e198: f32 = r_1;
    let _e201: f32 = w_2;
    let _e203: f32 = ext_1;
    let _e206: f32 = r_1;
    let _e208: f32 = w_2;
    let _e210: f32 = ext_1;
    return ((_e198 >= ((-0.5f * _e201) - _e203)) && (_e206 < ((0.5f * _e208) + _e210)));
}

fn cordWaveCovers(p_7: vec3<i32>, t_7: f32, xp_4: f32, amp_4: f32, sh_2: vec2<f32>) -> bool {
    var p_8: vec3<i32>;
    var t_8: f32;
    var xp_5: f32;
    var amp_5: f32;
    var sh_3: vec2<f32>;
    var dt_1: f32;
    var c_5: f32;
    var up_3: f32;
    var down_3: f32;

    p_8 = p_7;
    t_8 = t_7;
    xp_5 = xp_4;
    amp_5 = amp_4;
    sh_3 = sh_2;
    let _e182: i32 = S;
    dt_1 = (1f / f32(_e182));
    let _e186: vec3<i32> = p_8;
    let _e187: f32 = t_8;
    let _e188: f32 = amp_5;
    let _e189: vec2<f32> = sh_3;
    let _e190: f32 = cordWaveCentre(_e186, _e187, _e188, _e189);
    c_5 = _e190;
    let _e192: f32 = c_5;
    let _e193: vec3<i32> = p_8;
    let _e194: f32 = t_8;
    let _e195: f32 = dt_1;
    let _e197: f32 = amp_5;
    let _e198: vec2<f32> = sh_3;
    let _e199: f32 = cordWaveCentre(_e193, (_e194 - _e195), _e197, _e198);
    up_3 = abs((_e192 - _e199));
    let _e203: vec3<i32> = p_8;
    let _e204: f32 = t_8;
    let _e205: f32 = dt_1;
    let _e207: f32 = amp_5;
    let _e208: vec2<f32> = sh_3;
    let _e209: f32 = cordWaveCentre(_e203, (_e204 + _e205), _e207, _e208);
    let _e210: f32 = c_5;
    down_3 = abs((_e209 - _e210));
    let _e214: f32 = c_5;
    let _e215: f32 = up_3;
    let _e216: f32 = down_3;
    let _e217: f32 = xp_5;
    let _e218: bool = cordSpan(_e214, _e215, _e216, _e217);
    return _e218;
}

fn cordTexel(x_30: i32, y_24: i32, z_30: i32, dx_6: i32, dy_4: i32) -> bool {
    var x_31: i32;
    var y_25: i32;
    var z_31: i32;
    var dx_7: i32;
    var dy_5: i32;
    var t_9: f32;
    var e_1: vec2<f32>;

    x_31 = x_30;
    y_25 = y_24;
    z_31 = z_30;
    dx_7 = dx_6;
    dy_5 = dy_4;
    let _e181: bool = cordWave();
    if _e181 {
        {
            let _e182: i32 = dy_5;
            let _e186: i32 = S;
            t_9 = ((f32(_e182) + 0.5f) / f32(_e186));
            let _e190: i32 = x_31;
            let _e191: i32 = y_25;
            let _e192: i32 = z_31;
            let _e194: f32 = t_9;
            let _e195: i32 = dx_7;
            let _e200: f32 = cordAmp(true);
            let _e203: bool = cordWaveCovers(vec3<i32>(_e190, _e191, _e192), _e194, (f32(_e195) + 0.5f), _e200, vec2(0f));
            return _e203;
        }
    }
    let _e204: i32 = x_31;
    let _e205: i32 = y_25;
    let _e206: i32 = z_31;
    let _e207: vec2<f32> = cordEnds(_e204, _e205, _e206);
    e_1 = _e207;
    let _e209: vec2<f32> = e_1;
    let _e211: vec2<f32> = e_1;
    let _e213: i32 = dy_5;
    let _e217: i32 = S;
    let _e220: i32 = dx_7;
    let _e224: bool = cordCovers(_e209.x, _e211.y, ((f32(_e213) + 0.5f) / f32(_e217)), (f32(_e220) + 0.5f));
    return _e224;
}

fn cordGlyphTexel(v_19: vec4<u32>, dx_8: i32, row_3: i32) -> u32 {
    var v_20: vec4<u32>;
    var dx_9: i32;
    var row_4: i32;
    var local_7: u32;

    v_20 = v_19;
    dx_9 = dx_8;
    row_4 = row_3;
    let _e177: vec4<u32> = v_20;
    let _e178: bool = cordCell(_e177);
    if _e178 {
        let _e179: vec4<u32> = v_20;
        let _e180: i32 = partOf(_e179);
        let _e181: vec4<u32> = v_20;
        let _e182: i32 = glyphOf(_e181);
        let _e183: i32 = S;
        let _e186: i32 = S;
        let _e189: u32 = glyphTexel(_e180, _e182, (_e183 / 2i), (_e186 / 2i));
        local_7 = _e189;
    } else {
        let _e190: vec4<u32> = v_20;
        let _e191: i32 = partOf(_e190);
        let _e192: vec4<u32> = v_20;
        let _e193: i32 = glyphOf(_e192);
        let _e194: i32 = dx_9;
        let _e195: i32 = row_4;
        let _e196: u32 = glyphTexel(_e191, _e193, _e194, _e195);
        local_7 = _e196;
    }
    let _e198: u32 = local_7;
    return _e198;
}

fn strataOf(m_2: i32) -> vec3<f32> {
    var m_3: i32;

    m_3 = m_2;
    let _e173: i32 = m_3;
    if (_e173 == 0i) {
        {
            let _e176: VoxelScene = u;
            return _e176.skyC.xyz;
        }
    }
    let _e179: i32 = m_3;
    if (_e179 == 1i) {
        {
            let _e182: VoxelScene = u;
            return _e182.bedrockC.xyz;
        }
    }
    let _e185: i32 = m_3;
    if (_e185 == 2i) {
        {
            let _e188: VoxelScene = u;
            return _e188.rockC.xyz;
        }
    }
    let _e191: VoxelScene = u;
    return _e191.soilC.xyz;
}

fn holdsPore(m_4: i32) -> bool {
    var m_5: i32;

    m_5 = m_4;
    let _e173: i32 = m_5;
    let _e176: i32 = m_5;
    return ((_e173 == 2i) || (_e176 == 3i));
}

fn hazeAt(zf: f32) -> f32 {
    var zf_1: f32;

    zf_1 = zf;
    let _e173: i32 = D;
    if (_e173 <= 1i) {
        {
            return 0f;
        }
    }
    let _e177: VoxelScene = u;
    let _e183: f32 = zf_1;
    let _e184: i32 = D;
    return (clamp(_e177.knobs.x, 0f, 1f) * clamp((_e183 / f32((_e184 - 1i))), 0f, 1f));
}

fn hazed(c_6: vec3<f32>, t_10: f32) -> vec3<f32> {
    var c_7: vec3<f32>;
    var t_11: f32;

    c_7 = c_6;
    t_11 = t_10;
    let _e175: vec3<f32> = c_7;
    let _e176: VoxelScene = u;
    let _e179: f32 = t_11;
    return mix(_e175, _e176.hazeC.xyz, vec3(clamp(_e179, 0f, 1f)));
}

fn roofShade(gap: i32) -> f32 {
    var gap_1: i32;
    var t_12: f32;

    gap_1 = gap;
    let _e173: i32 = gap_1;
    if (_e173 == 0i) {
        {
            return 1f;
        }
    }
    let _e178: i32 = gap_1;
    let _e183: VoxelScene = u;
    t_12 = (1f - exp((-(f32((_e178 - 1i))) / _e183.roofK.y)));
    let _e190: VoxelScene = u;
    let _e194: VoxelScene = u;
    let _e198: f32 = t_12;
    return (_e190.roofK.x + ((1f - _e194.roofK.x) * _e198));
}

fn roofGap(x_32: i32, y_26: i32, z_32: i32) -> i32 {
    var x_33: i32;
    var y_27: i32;
    var z_33: i32;
    var yy: i32;

    x_33 = x_32;
    y_27 = y_26;
    z_33 = z_32;
    let _e177: VoxelScene = u;
    if (_e177.geom.w != 0i) {
        {
            let _e182: i32 = x_33;
            let _e183: i32 = wrapX(_e182);
            let _e184: i32 = y_27;
            let _e185: i32 = z_33;
            let _e188: vec4<u32> = textureLoad(wgpu_roofTex, vec3<i32>(_e183, _e184, _e185), 0i);
            return i32(_e188.x);
        }
    }
    let _e191: i32 = y_27;
    yy = (_e191 + 1i);
    loop {
        let _e195: i32 = yy;
        let _e196: i32 = H;
        if !((_e195 < _e196)) {
            break;
        }
        {
            let _e202: i32 = x_33;
            let _e203: i32 = yy;
            let _e204: i32 = z_33;
            let _e205: vec4<u32> = at(_e202, _e203, _e204);
            let _e206: bool = solidV(_e205);
            if _e206 {
                {
                    let _e207: i32 = yy;
                    let _e208: i32 = y_27;
                    return (_e207 - _e208);
                }
            }
        }
        continuing {
            let _e199: i32 = yy;
            yy = (_e199 + 1i);
        }
    }
    return 0i;
}

fn styleAt(style: i32, part_2: i32) -> vec3<f32> {
    var style_1: i32;
    var part_3: i32;

    style_1 = style;
    part_3 = part_2;
    let _e175: i32 = part_3;
    let _e176: i32 = style_1;
    let _e179: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(_e175, _e176), 0i);
    return _e179.xyz;
}

fn styleEmit(style_2: i32) -> vec3<f32> {
    var style_3: i32;

    style_3 = style_2;
    let _e174: i32 = style_3;
    let _e177: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(3i, _e174), 0i);
    let _e179: i32 = style_3;
    let _e182: i32 = style_3;
    let _e189: f32 = u.emitK[(_e182 >> 2u)][(_e179 & 3i)];
    return (_e177.xyz * _e189);
}

fn emitsWhole(pigmentAlpha: f32) -> bool {
    var pigmentAlpha_1: f32;

    pigmentAlpha_1 = pigmentAlpha;
    let _e173: f32 = pigmentAlpha_1;
    return (fract(_e173) > 0.125f);
}

fn cellEntry(x_34: i32, y_28: i32, z_34: i32) -> i32 {
    var x_35: i32;
    var y_29: i32;
    var z_35: i32;
    var cap: u32;
    var key: u32;
    var h_4: u32;
    var i: i32 = 0i;
    var c_8: vec4<u32>;

    x_35 = x_34;
    y_29 = y_28;
    z_35 = z_34;
    let _e178: VoxelScene = u;
    cap = (u32(_e178.view.w) & 1048575u);
    let _e187: u32 = cap;
    if (_e187 == 0u) {
        {
            return -1i;
        }
    }
    let _e192: i32 = z_35;
    let _e193: i32 = H;
    let _e195: i32 = y_29;
    let _e197: i32 = W;
    let _e199: i32 = x_35;
    let _e200: i32 = wrapX(_e199);
    key = (u32(((((_e192 * _e193) + _e195) * _e197) + _e200)) + 1u);
    let _e206: u32 = key;
    let _e210: u32 = cap;
    let _e218: u32 = cap;
    h_4 = (((_e206 * 2654435761u) >> u32((32i - min(i32(firstTrailingBit(_e210)), 31i)))) & (_e218 - 1u));
    loop {
        let _e225: i32 = i;
        if !((_e225 < 32i)) {
            break;
        }
        {
            let _e232: VoxelScene = u;
            let _e235: u32 = h_4;
            let _e239: vec4<u32> = global.emitCells[(_e232.view.z + i32(_e235))];
            c_8 = _e239;
            let _e241: vec4<u32> = c_8;
            let _e243: u32 = key;
            if (_e241.x == _e243) {
                {
                    let _e245: VoxelScene = u;
                    let _e248: u32 = h_4;
                    return (_e245.view.z + i32(_e248));
                }
            }
            let _e251: vec4<u32> = c_8;
            if (_e251.x == 0u) {
                {
                    return -1i;
                }
            }
            let _e257: u32 = h_4;
            let _e260: u32 = cap;
            h_4 = ((_e257 + 1u) & (_e260 - 1u));
        }
        continuing {
            let _e229: i32 = i;
            i = (_e229 + 1i);
        }
    }
    return -1i;
}

fn entryEmit(i_1: i32, e_2: ptr<function, vec3<f32>>, on: ptr<function, f32>) -> bool {
    var i_2: i32;
    var c_9: vec4<u32>;
    var rg: vec2<f32>;
    var bo: vec2<f32>;

    i_2 = i_1;
    let _e177: i32 = i_2;
    if (!(LIT) || (_e177 < 0i)) {
        {
            return false;
        }
    }
    let _e182: i32 = i_2;
    let _e184: vec4<u32> = global.emitCells[_e182];
    c_9 = _e184;
    let _e186: vec4<u32> = c_9;
    if ((_e186.w >> 31u) != 0u) {
        {
            return false;
        }
    }
    let _e194: vec4<u32> = c_9;
    rg = unpack2x16float(_e194.y);
    let _e198: vec4<u32> = c_9;
    bo = unpack2x16float(_e198.z);
    let _e202: vec2<f32> = rg;
    let _e203: vec2<f32> = bo;
    (*e_2) = vec3<f32>(_e202.x, _e202.y, _e203.x);
    let _e208: vec2<f32> = bo;
    (*on) = clamp(_e208.y, 0f, 1f);
    return true;
}

fn cellEmit(x_36: i32, y_30: i32, z_36: i32, e_3: ptr<function, vec3<f32>>, on_1: ptr<function, f32>) -> bool {
    var x_37: i32;
    var y_31: i32;
    var z_37: i32;

    x_37 = x_36;
    y_31 = y_30;
    z_37 = z_36;
    if !(LIT) {
        {
            return false;
        }
    }
    let _e182: i32 = x_37;
    let _e183: i32 = y_31;
    let _e184: i32 = z_37;
    let _e185: i32 = cellEntry(_e182, _e183, _e184);
    let _e190: bool = entryEmit(_e185, e_3, on_1);
    return _e190;
}

fn entryVineFrom(i_3: i32, v_21: ptr<function, vec4<u32>>) -> f32 {
    var i_4: i32;
    var w_3: u32;
    var s: u32;

    i_4 = i_3;
    let _e175: i32 = i_4;
    let _e178: VoxelScene = u;
    if ((_e175 < 0i) || ((u32(_e178.view.w) & EMIT_VINES) == 0u)) {
        {
            return 0f;
        }
    }
    let _e187: i32 = i_4;
    let _e189: vec4<u32> = global.emitCells[_e187];
    w_3 = _e189.w;
    let _e192: u32 = w_3;
    s = ((_e192 >> 16u) & 255u);
    let _e199: u32 = s;
    if (_e199 == 0u) {
        {
            return 0f;
        }
    }
    let _e204: u32 = w_3;
    (*v_21).z = (_e204 & 255u);
    let _e208: u32 = w_3;
    (*v_21).w = ((_e208 >> 8u) & 255u);
    let _e214: u32 = s;
    return (f32(_e214) / 255f);
}

fn plantSize(x_38: i32, y_32: i32, z_38: i32) -> f32 {
    var x_39: i32;
    var y_33: i32;
    var z_39: i32;
    var i_5: i32;
    var p_9: u32;
    var local_8: f32;

    x_39 = x_38;
    y_33 = y_32;
    z_39 = z_38;
    let _e180: VoxelScene = u;
    if (!(PLANT_LIGHT) || ((u32(_e180.view.w) & EMIT_PLANTS) == 0u)) {
        {
            return 1f;
        }
    }
    let _e189: i32 = x_39;
    let _e190: i32 = y_33;
    let _e191: i32 = z_39;
    let _e192: i32 = cellEntry(_e189, _e190, _e191);
    i_5 = _e192;
    let _e194: i32 = i_5;
    if (_e194 < 0i) {
        {
            return 1f;
        }
    }
    let _e198: i32 = i_5;
    let _e200: vec4<u32> = global.emitCells[_e198];
    p_9 = ((_e200.w >> 24u) & 127u);
    let _e208: u32 = p_9;
    if (_e208 == 0u) {
        local_8 = 1f;
    } else {
        let _e212: u32 = p_9;
        local_8 = (f32((_e212 - 1u)) / 125f);
    }
    let _e219: f32 = local_8;
    return _e219;
}

fn crownEdge(x_40: i32, y_34: i32, z_40: i32, dx_10: i32) -> f32 {
    var x_41: i32;
    var y_35: i32;
    var z_41: i32;
    var dx_11: i32;
    var i_6: i32;
    var w_4: u32;
    var local_9: u32;
    var e_4: u32;
    var local_10: f32;

    x_41 = x_40;
    y_35 = y_34;
    z_41 = z_40;
    dx_11 = dx_10;
    let _e182: VoxelScene = u;
    let _e190: i32 = dx_11;
    let _e193: i32 = dx_11;
    let _e194: i32 = S;
    if ((!(PLANT_LIGHT) || ((u32(_e182.view.w) & EMIT_PLANTS) == 0u)) || ((_e190 != 0i) && (_e193 != (_e194 - 1i)))) {
        {
            return 1f;
        }
    }
    let _e201: i32 = x_41;
    let _e202: i32 = y_35;
    let _e203: i32 = z_41;
    let _e204: i32 = cellEntry(_e201, _e202, _e203);
    i_6 = _e204;
    let _e206: i32 = i_6;
    if (_e206 < 0i) {
        {
            return 1f;
        }
    }
    let _e210: i32 = i_6;
    let _e212: vec4<u32> = global.emitCells[_e210];
    w_4 = _e212.w;
    let _e215: u32 = w_4;
    if (((_e215 >> 16u) & 255u) != 0u) {
        {
            return 1f;
        }
    }
    let _e224: i32 = dx_11;
    if (_e224 == 0i) {
        let _e227: u32 = w_4;
        local_9 = (_e227 & 255u);
    } else {
        let _e230: u32 = w_4;
        local_9 = ((_e230 >> 8u) & 255u);
    }
    let _e237: u32 = local_9;
    e_4 = _e237;
    let _e239: u32 = e_4;
    if (_e239 == 0u) {
        local_10 = 1f;
    } else {
        let _e243: u32 = e_4;
        local_10 = (f32((_e243 - 1u)) / 254f);
    }
    let _e250: f32 = local_10;
    return _e250;
}

fn emitterTrans(trans: vec3<f32>, drawn: vec3<f32>, haze: f32) -> vec3<f32> {
    var trans_1: vec3<f32>;
    var drawn_1: vec3<f32>;
    var haze_1: f32;

    trans_1 = trans;
    drawn_1 = drawn;
    haze_1 = haze;
    let _e179: vec3<f32> = trans_1;
    let _e180: vec3<f32> = drawn_1;
    let _e181: f32 = haze_1;
    let _e185: VoxelScene = u;
    return (-((_e179 * max((_e180 - (clamp(_e181, 0f, 1f) * _e185.hazeC.xyz)), vec3(0f)))) - vec3(1f));
}

fn vineEmits(t_13: vec4<f32>) -> bool {
    var t_14: vec4<f32>;

    t_14 = t_13;
    let _e176: vec4<f32> = t_14;
    let _e180: vec4<f32> = t_14;
    return ((_e176.w >= 0.5f) && (_e180.w < 0.998f));
}

fn plantLit(c_10: vec3<f32>, shade: f32) -> vec3<f32> {
    var c_11: vec3<f32>;
    var shade_1: f32;
    var lit: vec3<f32>;
    var local_11: vec3<f32>;

    c_11 = c_10;
    shade_1 = shade;
    let _e178: vec3<f32> = c_11;
    let _e179: VoxelScene = u;
    let _e183: VoxelScene = u;
    let _e186: VoxelScene = u;
    lit = mix((_e178 * _e179.plantA.x), _e183.lightC.xyz, vec3(_e186.plantA.y));
    let _e192: f32 = shade_1;
    if (_e192 < 1f) {
        let _e195: vec3<f32> = c_11;
        let _e196: vec3<f32> = lit;
        let _e197: f32 = shade_1;
        local_11 = mix(_e195, _e196, vec3(_e197));
    } else {
        let _e200: vec3<f32> = lit;
        local_11 = _e200;
    }
    let _e202: vec3<f32> = local_11;
    return _e202;
}

fn faceBase(m_6: i32, slot_4: i32, x_42: i32, y_36: i32, z_42: i32, salt_6: i32, dx_12: i32, dy_6: i32) -> vec3<f32> {
    var m_7: i32;
    var slot_5: i32;
    var x_43: i32;
    var y_37: i32;
    var z_43: i32;
    var salt_7: i32;
    var dx_13: i32;
    var dy_7: i32;

    m_7 = m_6;
    slot_5 = slot_4;
    x_43 = x_42;
    y_37 = y_36;
    z_43 = z_42;
    salt_7 = salt_6;
    dx_13 = dx_12;
    dy_7 = dy_6;
    let _e190: i32 = slot_5;
    let _e191: bool = texOn(_e190);
    if !(_e191) {
        {
            let _e193: i32 = m_7;
            let _e194: vec3<f32> = strataOf(_e193);
            return _e194;
        }
    }
    let _e195: i32 = slot_5;
    let _e196: i32 = x_43;
    let _e197: i32 = y_37;
    let _e198: i32 = z_43;
    let _e199: i32 = salt_7;
    let _e200: i32 = dx_13;
    let _e201: i32 = dy_7;
    let _e202: vec4<f32> = faceTexel(_e195, _e196, _e197, _e198, _e199, _e200, _e201);
    let _e204: vec3<f32> = srgbToLinear(_e202.xyz);
    return _e204;
}

fn blockBody(v_22: vec4<u32>, base: vec3<f32>) -> vec3<f32> {
    var v_23: vec4<u32>;
    var base_1: vec3<f32>;
    var m_8: i32;
    var local_12: f32;
    var wet: f32;

    v_23 = v_22;
    base_1 = base;
    let _e178: vec4<u32> = v_23;
    let _e179: i32 = matOf(_e178);
    m_8 = _e179;
    let _e181: i32 = m_8;
    let _e182: bool = holdsPore(_e181);
    if _e182 {
        let _e183: vec4<u32> = v_23;
        local_12 = clamp((f32(_e183.z) / 255f), 0f, 1f);
    } else {
        local_12 = 0f;
    }
    let _e193: f32 = local_12;
    wet = _e193;
    let _e195: vec3<f32> = base_1;
    let _e196: VoxelScene = u;
    let _e199: f32 = wet;
    let _e200: VoxelScene = u;
    return mix(_e195, _e196.waterDeepC.xyz, vec3((_e199 * _e200.shadeB.w)));
}

fn blockLit(body: vec3<f32>, shade_2: f32) -> vec3<f32> {
    var body_1: vec3<f32>;
    var shade_3: f32;
    var lit_1: vec3<f32>;
    var local_13: vec3<f32>;

    body_1 = body;
    shade_3 = shade_2;
    let _e178: vec3<f32> = body_1;
    let _e179: VoxelScene = u;
    let _e183: VoxelScene = u;
    let _e186: VoxelScene = u;
    lit_1 = mix((_e178 * _e179.shadeA.x), _e183.lightC.xyz, vec3(_e186.shadeA.y));
    let _e192: f32 = shade_3;
    if (_e192 < 1f) {
        let _e195: vec3<f32> = body_1;
        let _e196: vec3<f32> = lit_1;
        let _e197: f32 = shade_3;
        local_13 = mix(_e195, _e196, vec3(_e197));
    } else {
        let _e200: vec3<f32> = lit_1;
        local_13 = _e200;
    }
    let _e202: vec3<f32> = local_13;
    return _e202;
}

fn occluder(x_44: i32, y_38: i32, z_44: i32) -> f32 {
    var x_45: i32;
    var y_39: i32;
    var z_45: i32;
    var v_24: vec4<u32>;
    var local_14: f32;
    var p_10: i32;
    var local_15: f32;

    x_45 = x_44;
    y_39 = y_38;
    z_45 = z_44;
    let _e180: i32 = y_39;
    let _e181: bool = inY(_e180);
    let _e183: i32 = z_45;
    let _e187: i32 = z_45;
    let _e188: i32 = D;
    if ((!(_e181) || (_e183 < 0i)) || (_e187 >= _e188)) {
        {
            return 0f;
        }
    }
    let _e192: i32 = x_45;
    let _e193: i32 = y_39;
    let _e194: i32 = z_45;
    let _e195: vec4<u32> = at(_e192, _e193, _e194);
    v_24 = _e195;
    if !(PLANT_LIGHT) {
        {
            let _e198: vec4<u32> = v_24;
            let _e199: bool = solidV(_e198);
            let _e200: vec4<u32> = v_24;
            let _e201: i32 = partOf(_e200);
            let _e202: bool = isBlockPart(_e201);
            let _e203: vec4<u32> = v_24;
            let _e204: bool = softMark(_e203);
            let _e207: vec4<u32> = v_24;
            let _e208: bool = cordCell(_e207);
            if (_e199 || ((_e202 && !(_e204)) && !(_e208))) {
                local_14 = 1f;
            } else {
                local_14 = 0f;
            }
            let _e215: f32 = local_14;
            return _e215;
        }
    }
    let _e216: vec4<u32> = v_24;
    let _e217: bool = solidV(_e216);
    if _e217 {
        {
            return 1f;
        }
    }
    let _e219: vec4<u32> = v_24;
    let _e220: i32 = partOf(_e219);
    p_10 = _e220;
    let _e222: i32 = p_10;
    let _e223: bool = isBlockPart(_e222);
    let _e225: vec4<u32> = v_24;
    let _e226: bool = softMark(_e225);
    let _e228: vec4<u32> = v_24;
    let _e229: bool = cordCell(_e228);
    if ((!(_e223) || _e226) || _e229) {
        {
            return 0f;
        }
    }
    let _e232: i32 = p_10;
    let _e233: bool = plantBlockPart(_e232);
    if _e233 {
        let _e234: i32 = x_45;
        let _e235: i32 = y_39;
        let _e236: i32 = z_45;
        let _e237: f32 = plantSize(_e234, _e235, _e236);
        local_15 = _e237;
    } else {
        local_15 = 1f;
    }
    let _e240: f32 = local_15;
    return _e240;
}

fn occluderAt(c_12: vec3<i32>) -> f32 {
    var c_13: vec3<i32>;

    c_13 = c_12;
    let _e176: vec3<i32> = c_13;
    let _e178: vec3<i32> = c_13;
    let _e180: vec3<i32> = c_13;
    let _e182: f32 = occluder(_e176.x, _e178.y, _e180.z);
    return _e182;
}

fn creaseWidth(rows: i32) -> i32 {
    var rows_1: i32;

    rows_1 = rows;
    let _e177: i32 = S;
    let _e184: i32 = rows_1;
    return min(max(1i, ((_e177 + 4i) / 8i)), max(1i, (_e184 / 2i)));
}

fn creaseBands(l: f32, r_2: f32, lo_1: f32, hi: f32, cLL: f32, cRL: f32, cLH: f32, cRH: f32, du: i32, dv: i32, nu: i32, nv: i32, b_4: i32) -> f32 {
    var l_1: f32;
    var r_3: f32;
    var lo_2: f32;
    var hi_1: f32;
    var cLL_1: f32;
    var cRL_1: f32;
    var cLH_1: f32;
    var cRH_1: f32;
    var du_1: i32;
    var dv_1: i32;
    var nu_1: i32;
    var nv_1: i32;
    var b_5: i32;
    var inL: bool;
    var inR: bool;
    var inH: bool;
    var inLo: bool;
    var band: bool;
    var local_16: f32;
    var local_17: f32;
    var local_18: f32;
    var local_19: f32;
    var local_20: f32;
    var k_5: f32;
    var local_21: f32;
    var local_22: f32;
    var local_23: f32;
    var local_24: f32;

    l_1 = l;
    r_3 = r_2;
    lo_2 = lo_1;
    hi_1 = hi;
    cLL_1 = cLL;
    cRL_1 = cRL;
    cLH_1 = cLH;
    cRH_1 = cRH;
    du_1 = du;
    dv_1 = dv;
    nu_1 = nu;
    nv_1 = nv;
    b_5 = b_4;
    let _e200: i32 = du_1;
    let _e201: i32 = b_5;
    inL = (_e200 < _e201);
    let _e204: i32 = du_1;
    let _e205: i32 = nu_1;
    let _e206: i32 = b_5;
    inR = (_e204 >= (_e205 - _e206));
    let _e210: i32 = dv_1;
    let _e211: i32 = b_5;
    inH = (_e210 < _e211);
    let _e214: i32 = dv_1;
    let _e215: i32 = nv_1;
    let _e216: i32 = b_5;
    inLo = (_e214 >= (_e215 - _e216));
    if !(PLANT_LIGHT) {
        {
            let _e221: bool = inL;
            let _e222: f32 = l_1;
            let _e226: bool = inR;
            let _e227: f32 = r_3;
            let _e232: bool = inH;
            let _e233: f32 = hi_1;
            let _e238: bool = inLo;
            let _e239: f32 = lo_2;
            let _e244: bool = inL;
            let _e245: bool = inLo;
            let _e247: f32 = cLL_1;
            let _e252: bool = inR;
            let _e253: bool = inLo;
            let _e255: f32 = cRL_1;
            let _e260: bool = inL;
            let _e261: bool = inH;
            let _e263: f32 = cLH_1;
            let _e268: bool = inR;
            let _e269: bool = inH;
            let _e271: f32 = cRH_1;
            band = ((((((((_e221 && (_e222 > 0.5f)) || (_e226 && (_e227 > 0.5f))) || (_e232 && (_e233 > 0.5f))) || (_e238 && (_e239 > 0.5f))) || ((_e244 && _e245) && (_e247 > 0.5f))) || ((_e252 && _e253) && (_e255 > 0.5f))) || ((_e260 && _e261) && (_e263 > 0.5f))) || ((_e268 && _e269) && (_e271 > 0.5f)));
            let _e277: bool = band;
            if _e277 {
                let _e279: VoxelScene = u;
                local_16 = (1f - _e279.lightK.w);
            } else {
                local_16 = 1f;
            }
            let _e285: f32 = local_16;
            return _e285;
        }
    }
    let _e286: bool = inL;
    if _e286 {
        let _e287: f32 = l_1;
        local_17 = _e287;
    } else {
        local_17 = 0f;
    }
    let _e290: f32 = local_17;
    let _e291: bool = inR;
    if _e291 {
        let _e292: f32 = r_3;
        local_18 = _e292;
    } else {
        local_18 = 0f;
    }
    let _e295: f32 = local_18;
    let _e297: bool = inH;
    if _e297 {
        let _e298: f32 = hi_1;
        local_19 = _e298;
    } else {
        local_19 = 0f;
    }
    let _e301: f32 = local_19;
    let _e302: bool = inLo;
    if _e302 {
        let _e303: f32 = lo_2;
        local_20 = _e303;
    } else {
        local_20 = 0f;
    }
    let _e306: f32 = local_20;
    k_5 = max(max(_e290, _e295), max(_e301, _e306));
    let _e310: f32 = k_5;
    let _e311: bool = inL;
    let _e312: bool = inLo;
    if (_e311 && _e312) {
        let _e314: f32 = cLL_1;
        local_21 = _e314;
    } else {
        local_21 = 0f;
    }
    let _e317: f32 = local_21;
    let _e318: bool = inR;
    let _e319: bool = inLo;
    if (_e318 && _e319) {
        let _e321: f32 = cRL_1;
        local_22 = _e321;
    } else {
        local_22 = 0f;
    }
    let _e324: f32 = local_22;
    let _e326: bool = inL;
    let _e327: bool = inH;
    if (_e326 && _e327) {
        let _e329: f32 = cLH_1;
        local_23 = _e329;
    } else {
        local_23 = 0f;
    }
    let _e332: f32 = local_23;
    let _e333: bool = inR;
    let _e334: bool = inH;
    if (_e333 && _e334) {
        let _e336: f32 = cRH_1;
        local_24 = _e336;
    } else {
        local_24 = 0f;
    }
    let _e339: f32 = local_24;
    k_5 = max(_e310, max(max(_e317, _e324), max(_e332, _e339)));
    let _e344: VoxelScene = u;
    let _e347: f32 = k_5;
    return (1f - (_e344.lightK.w * _e347));
}

fn crease(l_2: f32, r_4: f32, lo_3: f32, hi_2: f32, cLL_2: f32, cRL_2: f32, cLH_2: f32, cRH_2: f32, du_2: i32, dv_2: i32, nu_2: i32, nv_2: i32) -> f32 {
    var l_3: f32;
    var r_5: f32;
    var lo_4: f32;
    var hi_3: f32;
    var cLL_3: f32;
    var cRL_3: f32;
    var cLH_3: f32;
    var cRH_3: f32;
    var du_3: i32;
    var dv_3: i32;
    var nu_3: i32;
    var nv_3: i32;

    l_3 = l_2;
    r_5 = r_4;
    lo_4 = lo_3;
    hi_3 = hi_2;
    cLL_3 = cLL_2;
    cRL_3 = cRL_2;
    cLH_3 = cLH_2;
    cRH_3 = cRH_2;
    du_3 = du_2;
    dv_3 = dv_2;
    nu_3 = nu_2;
    nv_3 = nv_2;
    let _e198: f32 = l_3;
    let _e199: f32 = r_5;
    let _e200: f32 = lo_4;
    let _e201: f32 = hi_3;
    let _e202: f32 = cLL_3;
    let _e203: f32 = cRL_3;
    let _e204: f32 = cLH_3;
    let _e205: f32 = cRH_3;
    let _e206: i32 = du_3;
    let _e207: i32 = dv_3;
    let _e208: i32 = nu_3;
    let _e209: i32 = nv_3;
    let _e210: i32 = nv_3;
    let _e211: i32 = creaseWidth(_e210);
    let _e212: f32 = creaseBands(_e198, _e199, _e200, _e201, _e202, _e203, _e204, _e205, _e206, _e207, _e208, _e209, _e211);
    return _e212;
}

fn creaseOpen(o_1: vec3<i32>, U: vec3<i32>, V: vec3<i32>, du_4: i32, dv_4: i32, nu_4: i32, nv_4: i32) -> f32 {
    var o_2: vec3<i32>;
    var U_1: vec3<i32>;
    var V_1: vec3<i32>;
    var du_5: i32;
    var dv_5: i32;
    var nu_5: i32;
    var nv_5: i32;
    var b_6: i32;
    var inL_1: bool;
    var inR_1: bool;
    var inH_1: bool;
    var inLo_1: bool;
    var k_6: f32 = 0f;

    o_2 = o_1;
    U_1 = U;
    V_1 = V;
    du_5 = du_4;
    dv_5 = dv_4;
    nu_5 = nu_4;
    nv_5 = nv_4;
    let _e188: i32 = nv_5;
    let _e189: i32 = creaseWidth(_e188);
    b_6 = _e189;
    let _e191: i32 = du_5;
    let _e192: i32 = b_6;
    inL_1 = (_e191 < _e192);
    let _e195: i32 = du_5;
    let _e196: i32 = nu_5;
    let _e197: i32 = b_6;
    inR_1 = (_e195 >= (_e196 - _e197));
    let _e201: i32 = dv_5;
    let _e202: i32 = b_6;
    inH_1 = (_e201 < _e202);
    let _e205: i32 = dv_5;
    let _e206: i32 = nv_5;
    let _e207: i32 = b_6;
    inLo_1 = (_e205 >= (_e206 - _e207));
    let _e213: bool = inL_1;
    if _e213 {
        {
            let _e214: f32 = k_6;
            let _e215: vec3<i32> = o_2;
            let _e216: vec3<i32> = U_1;
            let _e218: f32 = occluderAt((_e215 - _e216));
            k_6 = max(_e214, _e218);
        }
    }
    let _e220: bool = inR_1;
    if _e220 {
        {
            let _e221: f32 = k_6;
            let _e222: vec3<i32> = o_2;
            let _e223: vec3<i32> = U_1;
            let _e225: f32 = occluderAt((_e222 + _e223));
            k_6 = max(_e221, _e225);
        }
    }
    let _e227: bool = inH_1;
    if _e227 {
        {
            let _e228: f32 = k_6;
            let _e229: vec3<i32> = o_2;
            let _e230: vec3<i32> = V_1;
            let _e232: f32 = occluderAt((_e229 + _e230));
            k_6 = max(_e228, _e232);
        }
    }
    let _e234: bool = inLo_1;
    if _e234 {
        {
            let _e235: f32 = k_6;
            let _e236: vec3<i32> = o_2;
            let _e237: vec3<i32> = V_1;
            let _e239: f32 = occluderAt((_e236 - _e237));
            k_6 = max(_e235, _e239);
        }
    }
    let _e241: bool = inL_1;
    let _e242: bool = inLo_1;
    if (_e241 && _e242) {
        {
            let _e244: f32 = k_6;
            let _e245: vec3<i32> = o_2;
            let _e246: vec3<i32> = U_1;
            let _e248: vec3<i32> = V_1;
            let _e250: f32 = occluderAt(((_e245 - _e246) - _e248));
            k_6 = max(_e244, _e250);
        }
    }
    let _e252: bool = inR_1;
    let _e253: bool = inLo_1;
    if (_e252 && _e253) {
        {
            let _e255: f32 = k_6;
            let _e256: vec3<i32> = o_2;
            let _e257: vec3<i32> = U_1;
            let _e259: vec3<i32> = V_1;
            let _e261: f32 = occluderAt(((_e256 + _e257) - _e259));
            k_6 = max(_e255, _e261);
        }
    }
    let _e263: bool = inL_1;
    let _e264: bool = inH_1;
    if (_e263 && _e264) {
        {
            let _e266: f32 = k_6;
            let _e267: vec3<i32> = o_2;
            let _e268: vec3<i32> = U_1;
            let _e270: vec3<i32> = V_1;
            let _e272: f32 = occluderAt(((_e267 - _e268) + _e270));
            k_6 = max(_e266, _e272);
        }
    }
    let _e274: bool = inR_1;
    let _e275: bool = inH_1;
    if (_e274 && _e275) {
        {
            let _e277: f32 = k_6;
            let _e278: vec3<i32> = o_2;
            let _e279: vec3<i32> = U_1;
            let _e281: vec3<i32> = V_1;
            let _e283: f32 = occluderAt(((_e278 + _e279) + _e281));
            k_6 = max(_e277, _e283);
        }
    }
    let _e286: VoxelScene = u;
    let _e289: f32 = k_6;
    return (1f - (_e286.lightK.w * _e289));
}

fn modelCrease(mask: u32, dx_14: i32, dy_8: i32, size: vec2<i32>) -> f32 {
    var mask_1: u32;
    var dx_15: i32;
    var dy_9: i32;
    var size_1: vec2<i32>;

    mask_1 = mask;
    dx_15 = dx_14;
    dy_9 = dy_8;
    size_1 = size;
    if !(AO) {
        {
            return 1f;
        }
    }
    let _e184: u32 = mask_1;
    let _e188: u32 = mask_1;
    let _e195: u32 = mask_1;
    let _e202: u32 = mask_1;
    let _e209: u32 = mask_1;
    let _e216: u32 = mask_1;
    let _e223: u32 = mask_1;
    let _e230: u32 = mask_1;
    let _e237: i32 = dx_15;
    let _e238: i32 = dy_9;
    let _e239: vec2<i32> = size_1;
    let _e241: vec2<i32> = size_1;
    let _e243: vec2<i32> = size_1;
    let _e245: vec2<i32> = size_1;
    let _e248: i32 = creaseWidth(min(_e243.x, _e245.y));
    let _e249: f32 = creaseBands(f32((_e184 & 1u)), f32(((_e188 >> 1u) & 1u)), f32(((_e195 >> 2u) & 1u)), f32(((_e202 >> 3u) & 1u)), f32(((_e209 >> 4u) & 1u)), f32(((_e216 >> 5u) & 1u)), f32(((_e223 >> 6u) & 1u)), f32(((_e230 >> 7u) & 1u)), _e237, _e238, _e239.x, _e241.y, _e248);
    return _e249;
}

fn aoFront(x_46: i32, y_40: i32, z_46: i32, dx_16: i32, dy_10: i32) -> f32 {
    var x_47: i32;
    var y_41: i32;
    var z_47: i32;
    var dx_17: i32;
    var dy_11: i32;
    var p_11: i32;

    x_47 = x_46;
    y_41 = y_40;
    z_47 = z_46;
    dx_17 = dx_16;
    dy_11 = dy_10;
    if !(AO) {
        {
            return 1f;
        }
    }
    if !(PLANT_LIGHT) {
        {
            let _e187: i32 = z_47;
            p_11 = (_e187 - 1i);
            let _e191: i32 = x_47;
            let _e194: i32 = y_41;
            let _e195: i32 = p_11;
            let _e196: f32 = occluder((_e191 - 1i), _e194, _e195);
            let _e197: i32 = x_47;
            let _e200: i32 = y_41;
            let _e201: i32 = p_11;
            let _e202: f32 = occluder((_e197 + 1i), _e200, _e201);
            let _e203: i32 = x_47;
            let _e204: i32 = y_41;
            let _e207: i32 = p_11;
            let _e208: f32 = occluder(_e203, (_e204 - 1i), _e207);
            let _e209: i32 = x_47;
            let _e210: i32 = y_41;
            let _e213: i32 = p_11;
            let _e214: f32 = occluder(_e209, (_e210 + 1i), _e213);
            let _e215: i32 = x_47;
            let _e218: i32 = y_41;
            let _e221: i32 = p_11;
            let _e222: f32 = occluder((_e215 - 1i), (_e218 - 1i), _e221);
            let _e223: i32 = x_47;
            let _e226: i32 = y_41;
            let _e229: i32 = p_11;
            let _e230: f32 = occluder((_e223 + 1i), (_e226 - 1i), _e229);
            let _e231: i32 = x_47;
            let _e234: i32 = y_41;
            let _e237: i32 = p_11;
            let _e238: f32 = occluder((_e231 - 1i), (_e234 + 1i), _e237);
            let _e239: i32 = x_47;
            let _e242: i32 = y_41;
            let _e245: i32 = p_11;
            let _e246: f32 = occluder((_e239 + 1i), (_e242 + 1i), _e245);
            let _e247: i32 = dx_17;
            let _e248: i32 = dy_11;
            let _e249: i32 = S;
            let _e250: i32 = S;
            let _e251: f32 = crease(_e196, _e202, _e208, _e214, _e222, _e230, _e238, _e246, _e247, _e248, _e249, _e250);
            return _e251;
        }
    }
    let _e252: i32 = x_47;
    let _e253: i32 = y_41;
    let _e254: i32 = z_47;
    let _e266: i32 = dx_17;
    let _e267: i32 = dy_11;
    let _e268: i32 = S;
    let _e269: i32 = S;
    let _e270: f32 = creaseOpen(vec3<i32>(_e252, _e253, (_e254 - 1i)), vec3<i32>(1i, 0i, 0i), vec3<i32>(0i, 1i, 0i), _e266, _e267, _e268, _e269);
    return _e270;
}

fn aoTop(x_48: i32, y_42: i32, z_48: i32, dx_18: i32, dy_12: i32) -> f32 {
    var x_49: i32;
    var y_43: i32;
    var z_49: i32;
    var dx_19: i32;
    var dy_13: i32;
    var p_12: i32;

    x_49 = x_48;
    y_43 = y_42;
    z_49 = z_48;
    dx_19 = dx_18;
    dy_13 = dy_12;
    if !(AO) {
        {
            return 1f;
        }
    }
    if !(PLANT_LIGHT) {
        {
            let _e187: i32 = y_43;
            p_12 = (_e187 + 1i);
            let _e191: i32 = x_49;
            let _e194: i32 = p_12;
            let _e195: i32 = z_49;
            let _e196: f32 = occluder((_e191 - 1i), _e194, _e195);
            let _e197: i32 = x_49;
            let _e200: i32 = p_12;
            let _e201: i32 = z_49;
            let _e202: f32 = occluder((_e197 + 1i), _e200, _e201);
            let _e203: i32 = x_49;
            let _e204: i32 = p_12;
            let _e205: i32 = z_49;
            let _e208: f32 = occluder(_e203, _e204, (_e205 - 1i));
            let _e209: i32 = x_49;
            let _e210: i32 = p_12;
            let _e211: i32 = z_49;
            let _e214: f32 = occluder(_e209, _e210, (_e211 + 1i));
            let _e215: i32 = x_49;
            let _e218: i32 = p_12;
            let _e219: i32 = z_49;
            let _e222: f32 = occluder((_e215 - 1i), _e218, (_e219 - 1i));
            let _e223: i32 = x_49;
            let _e226: i32 = p_12;
            let _e227: i32 = z_49;
            let _e230: f32 = occluder((_e223 + 1i), _e226, (_e227 - 1i));
            let _e231: i32 = x_49;
            let _e234: i32 = p_12;
            let _e235: i32 = z_49;
            let _e238: f32 = occluder((_e231 - 1i), _e234, (_e235 + 1i));
            let _e239: i32 = x_49;
            let _e242: i32 = p_12;
            let _e243: i32 = z_49;
            let _e246: f32 = occluder((_e239 + 1i), _e242, (_e243 + 1i));
            let _e247: i32 = dx_19;
            let _e248: i32 = dy_13;
            let _e249: i32 = S;
            let _e250: i32 = RISE;
            let _e251: f32 = crease(_e196, _e202, _e208, _e214, _e222, _e230, _e238, _e246, _e247, _e248, _e249, _e250);
            return _e251;
        }
    }
    let _e252: i32 = x_49;
    let _e253: i32 = y_43;
    let _e256: i32 = z_49;
    let _e266: i32 = dx_19;
    let _e267: i32 = dy_13;
    let _e268: i32 = S;
    let _e269: i32 = RISE;
    let _e270: f32 = creaseOpen(vec3<i32>(_e252, (_e253 + 1i), _e256), vec3<i32>(1i, 0i, 0i), vec3<i32>(0i, 0i, 1i), _e266, _e267, _e268, _e269);
    return _e270;
}

fn skyOpen(x_50: i32, y_44: i32, z_50: i32) -> f32 {
    var x_51: i32;
    var y_45: i32;
    var z_51: i32;

    x_51 = x_50;
    y_45 = y_44;
    z_51 = z_50;
    let _e180: i32 = y_45;
    let _e181: i32 = H;
    let _e183: i32 = z_51;
    if ((_e180 >= _e181) || (_e183 < 0i)) {
        {
            return 1f;
        }
    }
    let _e188: i32 = y_45;
    let _e191: i32 = z_51;
    let _e192: i32 = D;
    if ((_e188 < 0i) || (_e191 >= _e192)) {
        {
            return 0f;
        }
    }
    let _e196: i32 = x_51;
    let _e197: i32 = wrapX(_e196);
    let _e198: i32 = y_45;
    let _e199: i32 = z_51;
    let _e202: vec4<u32> = textureLoad(wgpu_skyTex, vec3<i32>(_e197, _e198, _e199), 0i);
    return (f32(_e202.x) / 255f);
}

fn canopyAt(x_52: i32, z_52: i32, yv: i32) -> f32 {
    var x_53: i32;
    var z_53: i32;
    var yv_1: i32;
    var a_8: vec4<u32>;
    var b_7: vec4<u32>;

    x_53 = x_52;
    z_53 = z_52;
    yv_1 = yv;
    let _e180: i32 = z_53;
    let _e183: i32 = z_53;
    let _e184: i32 = D;
    if ((_e180 < 0i) || (_e183 >= _e184)) {
        {
            return 1f;
        }
    }
    let _e188: i32 = x_53;
    let _e189: i32 = wrapX(_e188);
    let _e191: i32 = z_53;
    let _e195: vec4<u32> = textureLoad(wgpu_canopyTex, vec2<i32>(_e189, (2i * _e191)), 0i);
    a_8 = _e195;
    let _e197: vec4<u32> = a_8;
    let _e200: i32 = yv_1;
    if (i32(_e197.x) <= _e200) {
        {
            return 1f;
        }
    }
    let _e203: vec4<u32> = a_8;
    let _e206: i32 = yv_1;
    if (i32(_e203.z) <= _e206) {
        {
            let _e208: vec4<u32> = a_8;
            return (f32(_e208.y) / 255f);
        }
    }
    let _e213: i32 = x_53;
    let _e214: i32 = wrapX(_e213);
    let _e216: i32 = z_53;
    let _e222: vec4<u32> = textureLoad(wgpu_canopyTex, vec2<i32>(_e214, ((2i * _e216) + 1i)), 0i);
    b_7 = _e222;
    let _e224: vec4<u32> = b_7;
    let _e227: i32 = yv_1;
    if (i32(_e224.x) <= _e227) {
        {
            let _e229: vec4<u32> = a_8;
            return (f32(_e229.w) / 255f);
        }
    }
    let _e234: vec4<u32> = b_7;
    let _e237: i32 = yv_1;
    if (i32(_e234.z) <= _e237) {
        {
            let _e239: vec4<u32> = b_7;
            return (f32(_e239.y) / 255f);
        }
    }
    let _e244: vec4<u32> = b_7;
    return (f32(_e244.w) / 255f);
}

fn plantCanopy(v_25: vec4<u32>) -> f32 {
    var v_26: vec4<u32>;

    v_26 = v_25;
    let _e176: vec4<u32> = v_26;
    return (f32((_e176.z & 15u)) / 15f);
}

fn crownPass(v_27: vec4<u32>) -> f32 {
    var v_28: vec4<u32>;

    v_28 = v_27;
    let _e176: vec4<u32> = v_28;
    return (f32((_e176.z >> 4u)) / 15f);
}

fn ladderLight(a_9: f32, sun: f32, local_25: vec3<f32>) -> vec3<f32> {
    var a_10: f32;
    var sun_1: f32;
    var local_26: vec3<f32>;
    var smoothLight: bool;
    var n_1: f32;
    var hue: vec3<f32>;
    var l_4: f32;
    var sum_1: f32;
    var local_27: f32;
    var share: f32;
    var local_28: f32;
    var t_15: f32;

    a_10 = a_9;
    sun_1 = sun;
    local_26 = local_25;
    let _e181: VoxelScene = u;
    smoothLight = (_e181.lightK.z < 1.5f);
    let _e187: VoxelScene = u;
    n_1 = max((_e187.lightK.z - 1f), 1f);
    let _e195: VoxelScene = u;
    hue = _e195.ambientC.xyz;
    let _e199: vec3<f32> = local_26;
    l_4 = dot(_e199, vec3<f32>(0.2126f, 0.7152f, 0.0722f));
    let _e206: f32 = l_4;
    if (_e206 > 0.001953125f) {
        {
            let _e211: f32 = a_10;
            let _e215: f32 = l_4;
            sum_1 = (clamp(_e211, 0f, 1f) + _e215);
            let _e218: bool = smoothLight;
            if _e218 {
                let _e219: f32 = l_4;
                let _e220: f32 = sum_1;
                local_27 = (_e219 / _e220);
            } else {
                let _e222: f32 = l_4;
                let _e223: f32 = sum_1;
                let _e225: f32 = n_1;
                let _e230: f32 = n_1;
                local_27 = (floor((((_e222 / _e223) * _e225) + 0.5f)) / _e230);
            }
            let _e233: f32 = local_27;
            share = _e233;
            let _e235: vec3<f32> = hue;
            let _e236: vec3<f32> = local_26;
            let _e237: f32 = l_4;
            let _e240: f32 = share;
            hue = mix(_e235, (_e236 / vec3(_e237)), vec3(_e240));
            let _e243: f32 = sum_1;
            a_10 = _e243;
        }
    }
    let _e244: bool = smoothLight;
    if _e244 {
        let _e245: f32 = a_10;
        let _e249: f32 = sun_1;
        let _e250: VoxelScene = u;
        local_28 = (clamp(_e245, 0f, 1f) + (_e249 * _e250.dayK.y));
    } else {
        let _e255: f32 = a_10;
        let _e259: f32 = n_1;
        let _e264: f32 = sun_1;
        let _e265: VoxelScene = u;
        let _e270: f32 = n_1;
        local_28 = ((floor(((clamp(_e255, 0f, 1f) * _e259) + 0.5f)) + (_e264 * _e265.dayK.x)) / _e270);
    }
    let _e273: f32 = local_28;
    t_15 = _e273;
    let _e275: vec3<f32> = hue;
    let _e276: VoxelScene = u;
    let _e279: VoxelScene = u;
    let _e283: VoxelScene = u;
    let _e287: f32 = t_15;
    return (_e275 * (_e276.lightK.x * (_e279.lightK.y + ((1f - _e283.lightK.y) * _e287))));
}

fn ladderRef(sunOn: f32) -> vec3<f32> {
    var sunOn_1: f32;
    var smoothLight_1: bool;
    var n_2: f32;
    var local_29: f32;
    var t_16: f32;

    sunOn_1 = sunOn;
    let _e177: VoxelScene = u;
    smoothLight_1 = (_e177.lightK.z < 1.5f);
    let _e183: VoxelScene = u;
    n_2 = max((_e183.lightK.z - 1f), 1f);
    let _e191: bool = smoothLight_1;
    if _e191 {
        let _e193: f32 = sunOn_1;
        local_29 = (1f + (_e193 * SMOOTH_SUN));
    } else {
        let _e196: f32 = n_2;
        let _e200: f32 = sunOn_1;
        let _e202: f32 = n_2;
        local_29 = ((floor((_e196 + 0.5f)) + _e200) / _e202);
    }
    let _e205: f32 = local_29;
    t_16 = _e205;
    let _e207: VoxelScene = u;
    let _e210: VoxelScene = u;
    let _e215: VoxelScene = u;
    let _e218: VoxelScene = u;
    let _e222: VoxelScene = u;
    let _e226: f32 = t_16;
    return ((_e207.ambientC.xyz / vec3(_e210.dayK.z)) * (_e215.lightK.x * (_e218.lightK.y + ((1f - _e222.lightK.y) * _e226))));
}

fn glowAt(p_13: vec3<f32>) -> vec3<f32> {
    var p_14: vec3<f32>;
    var n_3: vec3<f32>;

    p_14 = p_13;
    if !(GLOW) {
        {
            return vec3(0f);
        }
    }
    let _e181: vec3<u32> = textureDimensions(wgpu_glowTex, 0i);
    n_3 = (vec3<f32>(vec3<i32>(_e181)) * 4f);
    let _e187: vec3<f32> = p_14;
    let _e188: vec3<f32> = n_3;
    let _e191: vec4<f32> = textureSampleLevel(wgpu_glowTex, wgpu_sampler_0_33_, (_e187 / _e188), 0f);
    return _e191.xyz;
}

fn crownLets(v_29: vec4<u32>, c_14: vec3<i32>) -> bool {
    var v_30: vec4<u32>;
    var c_15: vec3<i32>;
    var h_5: u32;

    v_30 = v_29;
    c_15 = c_14;
    let _e180: vec3<i32> = c_15;
    let _e182: i32 = wrapX(_e180.x);
    let _e183: vec3<i32> = c_15;
    let _e185: vec3<i32> = c_15;
    let _e188: u32 = cellHash(_e182, _e183.y, _e185.z, 32i);
    h_5 = _e188;
    let _e190: u32 = h_5;
    let _e196: vec4<u32> = v_30;
    let _e197: f32 = crownPass(_e196);
    return ((f32((_e190 & 65535u)) / 65536f) < _e197);
}

fn figureShades(p_15: vec3<f32>) -> bool {
    var p_16: vec3<f32>;
    var fx: f32;
    var fz: f32;
    var t_17: vec2<f32>;
    var top: f32;

    p_16 = p_15;
    let _e178: VoxelScene = u;
    if (_e178.figSun.w < 0.5f) {
        {
            return false;
        }
    }
    let _e184: vec3<f32> = p_16;
    let _e186: vec3<f32> = p_16;
    let _e188: VoxelScene = u;
    fx = (_e184.x - (_e186.y * _e188.figSun.x));
    let _e194: vec3<f32> = p_16;
    let _e196: vec3<f32> = p_16;
    let _e198: VoxelScene = u;
    let _e203: VoxelScene = u;
    fz = ((_e194.z - (_e196.y * _e198.figSun.y)) - _e203.figSun.z);
    let _e208: f32 = fx;
    let _e209: i32 = W;
    let _e211: f32 = fx;
    let _e212: i32 = W;
    let _e218: VoxelScene = u;
    let _e222: f32 = fz;
    let _e223: VoxelScene = u;
    t_17 = vec2<f32>(((_e208 - (f32(_e209) * floor((_e211 / f32(_e212))))) * _e218.figMap.x), (_e222 * _e223.figMap.y));
    let _e229: vec2<f32> = t_17;
    let _e233: vec2<f32> = t_17;
    let _e235: VoxelScene = u;
    if ((_e229.y < 0f) || (_e233.y >= _e235.figMap.w)) {
        {
            return false;
        }
    }
    let _e241: vec2<f32> = t_17;
    let _e242: VoxelScene = u;
    let _e251: f32 = textureLoad(wgpu_figSunTex, vec2<i32>(min(_e241, (_e242.figMap.zw - vec2(1f)))), 0i);
    let _e253: i32 = H;
    top = (f32(_e251) * f32(_e253));
    let _e257: f32 = top;
    let _e258: vec3<f32> = p_16;
    return (_e257 > (_e258.y + 0.25f));
}

fn sunReaches(p_17: vec3<f32>, c_16: vec3<i32>) -> f32 {
    var p_18: vec3<f32>;
    var c_17: vec3<i32>;
    var L: vec3<f32>;
    var dir: vec3<i32>;
    var local_30: f32;
    var local_31: f32;
    var local_32: f32;
    var inv: vec3<f32>;
    var local_33: f32;
    var local_34: f32;
    var local_35: f32;
    var next: vec3<f32>;
    var pass_: f32 = 1f;
    var i_7: i32 = 0i;
    var local_36: i32;
    var local_37: i32;
    var axis: i32;
    var v_31: vec4<u32>;
    var part_4: i32;

    p_18 = p_17;
    c_17 = c_16;
    if !(SHADOWS) {
        {
            return 1f;
        }
    }
    let _e182: vec3<f32> = p_18;
    let _e183: bool = figureShades(_e182);
    if _e183 {
        {
            return 0f;
        }
    }
    let _e185: VoxelScene = u;
    L = _e185.sunK.xyz;
    let _e189: vec3<f32> = L;
    dir = vec3<i32>(sign(_e189));
    let _e193: vec3<f32> = L;
    if (abs(_e193.x) > 0.000001f) {
        let _e199: vec3<f32> = L;
        local_30 = (1f / abs(_e199.x));
    } else {
        local_30 = 1000000000000000000000000000000f;
    }
    let _e205: f32 = local_30;
    let _e206: vec3<f32> = L;
    if (abs(_e206.y) > 0.000001f) {
        let _e212: vec3<f32> = L;
        local_31 = (1f / abs(_e212.y));
    } else {
        local_31 = 1000000000000000000000000000000f;
    }
    let _e218: f32 = local_31;
    let _e219: vec3<f32> = L;
    if (abs(_e219.z) > 0.000001f) {
        let _e225: vec3<f32> = L;
        local_32 = (1f / abs(_e225.z));
    } else {
        local_32 = 1000000000000000000000000000000f;
    }
    let _e231: f32 = local_32;
    inv = vec3<f32>(_e205, _e218, _e231);
    let _e234: vec3<i32> = dir;
    if (_e234.x > 0i) {
        let _e238: vec3<i32> = c_17;
        let _e243: vec3<f32> = p_18;
        local_33 = (f32((_e238.x + 1i)) - _e243.x);
    } else {
        let _e246: vec3<f32> = p_18;
        let _e248: vec3<i32> = c_17;
        local_33 = (_e246.x - f32(_e248.x));
    }
    let _e253: f32 = local_33;
    let _e254: vec3<f32> = inv;
    let _e257: vec3<i32> = dir;
    if (_e257.y > 0i) {
        let _e261: vec3<i32> = c_17;
        let _e266: vec3<f32> = p_18;
        local_34 = (f32((_e261.y + 1i)) - _e266.y);
    } else {
        let _e269: vec3<f32> = p_18;
        let _e271: vec3<i32> = c_17;
        local_34 = (_e269.y - f32(_e271.y));
    }
    let _e276: f32 = local_34;
    let _e277: vec3<f32> = inv;
    let _e280: vec3<i32> = dir;
    if (_e280.z > 0i) {
        let _e284: vec3<i32> = c_17;
        let _e289: vec3<f32> = p_18;
        local_35 = (f32((_e284.z + 1i)) - _e289.z);
    } else {
        let _e292: vec3<f32> = p_18;
        let _e294: vec3<i32> = c_17;
        local_35 = (_e292.z - f32(_e294.z));
    }
    let _e299: f32 = local_35;
    let _e300: vec3<f32> = inv;
    next = vec3<f32>((_e253 * _e254.x), (_e276 * _e277.y), (_e299 * _e300.z));
    loop {
        let _e309: i32 = i_7;
        if !((_e309 < SUN_MARCH)) {
            break;
        }
        {
            let _e315: vec3<i32> = c_17;
            let _e317: i32 = H;
            let _e319: vec3<i32> = c_17;
            let _e324: vec3<i32> = c_17;
            let _e326: i32 = D;
            if (((_e315.y >= _e317) || (_e319.z < 0i)) || (_e324.z >= _e326)) {
                {
                    let _e329: f32 = pass_;
                    return _e329;
                }
            }
            let _e330: vec3<i32> = c_17;
            if (_e330.y < 0i) {
                {
                    return 0f;
                }
            }
            let _e335: vec3<f32> = next;
            let _e337: vec3<f32> = next;
            let _e340: vec3<f32> = next;
            let _e342: vec3<f32> = next;
            if ((_e335.x < _e337.y) && (_e340.x < _e342.z)) {
                local_37 = 0i;
            } else {
                let _e347: vec3<f32> = next;
                let _e349: vec3<f32> = next;
                if (_e347.y < _e349.z) {
                    local_36 = 1i;
                } else {
                    local_36 = 2i;
                }
                let _e355: i32 = local_36;
                local_37 = _e355;
            }
            let _e357: i32 = local_37;
            axis = _e357;
            let _e359: vec3<i32> = c_17;
            let _e361: vec3<i32> = c_17;
            let _e363: vec3<i32> = c_17;
            let _e365: vec4<u32> = at(_e359.x, _e361.y, _e363.z);
            v_31 = _e365;
            let _e367: vec4<u32> = v_31;
            let _e368: bool = solidV(_e367);
            if _e368 {
                {
                    return 0f;
                }
            }
            let _e370: vec4<u32> = v_31;
            let _e371: i32 = partOf(_e370);
            part_4 = _e371;
            if !(PLANT_LIGHT) {
                {
                    let _e374: i32 = part_4;
                    let _e376: i32 = part_4;
                    let _e379: i32 = part_4;
                    let _e381: vec4<u32> = v_31;
                    let _e382: bool = softMark(_e381);
                    if (((_e374 == TRUNK) || (_e376 == LOG)) || ((_e379 == ANIMAL_INTERIM) && !(_e382))) {
                        {
                            return 0f;
                        }
                    }
                    let _e387: i32 = part_4;
                    let _e389: i32 = part_4;
                    let _e392: vec4<u32> = v_31;
                    let _e393: vec3<i32> = c_17;
                    let _e394: bool = crownLets(_e392, _e393);
                    let _e397: vec4<u32> = v_31;
                    let _e398: bool = cordCell(_e397);
                    if ((((_e387 == CROWN) || (_e389 == CROWN_HEART)) && !(_e394)) && !(_e398)) {
                        {
                            return 0f;
                        }
                    }
                }
            } else {
                {
                    let _e402: i32 = part_4;
                    let _e404: i32 = part_4;
                    let _e406: vec4<u32> = v_31;
                    let _e407: bool = softMark(_e406);
                    if ((_e402 == LOG) || ((_e404 == ANIMAL_INTERIM) && !(_e407))) {
                        {
                            return 0f;
                        }
                    }
                    let _e412: i32 = part_4;
                    let _e414: i32 = part_4;
                    let _e416: i32 = part_4;
                    let _e419: vec4<u32> = v_31;
                    let _e420: vec3<i32> = c_17;
                    let _e421: bool = crownLets(_e419, _e420);
                    let _e424: vec4<u32> = v_31;
                    let _e425: bool = cordCell(_e424);
                    if ((_e412 == TRUNK) || ((((_e414 == CROWN) || (_e416 == CROWN_HEART)) && !(_e421)) && !(_e425))) {
                        {
                            let _e429: f32 = pass_;
                            let _e431: vec3<i32> = c_17;
                            let _e433: vec3<i32> = c_17;
                            let _e435: vec3<i32> = c_17;
                            let _e437: f32 = plantSize(_e431.x, _e433.y, _e435.z);
                            pass_ = (_e429 * (1f - _e437));
                            let _e440: f32 = pass_;
                            if (_e440 <= 0f) {
                                {
                                    return 0f;
                                }
                            }
                        }
                    }
                }
            }
            let _e444: i32 = axis;
            if (_e444 == 0i) {
                {
                    let _e448: vec3<i32> = c_17;
                    let _e450: vec3<i32> = dir;
                    c_17.x = (_e448.x + _e450.x);
                    let _e454: vec3<f32> = next;
                    let _e456: vec3<f32> = inv;
                    next.x = (_e454.x + _e456.x);
                }
            } else {
                let _e459: i32 = axis;
                if (_e459 == 1i) {
                    {
                        let _e463: vec3<i32> = c_17;
                        let _e465: vec3<i32> = dir;
                        c_17.y = (_e463.y + _e465.y);
                        let _e469: vec3<f32> = next;
                        let _e471: vec3<f32> = inv;
                        next.y = (_e469.y + _e471.y);
                    }
                } else {
                    {
                        let _e475: vec3<i32> = c_17;
                        let _e477: vec3<i32> = dir;
                        c_17.z = (_e475.z + _e477.z);
                        let _e481: vec3<f32> = next;
                        let _e483: vec3<f32> = inv;
                        next.z = (_e481.z + _e483.z);
                    }
                }
            }
        }
        continuing {
            let _e312: i32 = i_7;
            i_7 = (_e312 + 1i);
        }
    }
    let _e486: f32 = pass_;
    return _e486;
}

fn litBy(base_2: vec3<f32>, ambient: f32, sun_2: f32, ndl: f32, local_38: vec3<f32>) -> vec3<f32> {
    var base_3: vec3<f32>;
    var ambient_1: f32;
    var sun_3: f32;
    var ndl_1: f32;
    var local_39: vec3<f32>;
    var c_18: vec3<f32>;

    base_3 = base_2;
    ambient_1 = ambient;
    sun_3 = sun_2;
    ndl_1 = ndl;
    local_39 = local_38;
    let _e186: vec3<f32> = base_3;
    let _e187: f32 = ambient_1;
    let _e188: f32 = sun_3;
    let _e189: vec3<f32> = local_39;
    let _e190: vec3<f32> = ladderLight(_e187, _e188, _e189);
    c_18 = (_e186 * _e190);
    let _e193: vec3<f32> = c_18;
    let _e194: VoxelScene = u;
    let _e197: VoxelScene = u;
    let _e200: f32 = sun_3;
    let _e202: f32 = ndl_1;
    return mix(_e193, _e194.sunLean.xyz, vec3(clamp(((_e197.sunK.w * _e200) * _e202), 0f, 1f)));
}

fn shadeFront(base_4: vec3<f32>, x_54: i32, y_46: i32, z_54: i32, dx_20: i32, dy_14: i32, canopy: f32) -> vec3<f32> {
    var base_5: vec3<f32>;
    var x_55: i32;
    var y_47: i32;
    var z_55: i32;
    var dx_21: i32;
    var dy_15: i32;
    var canopy_1: f32;
    var ndl_2: f32;
    var sun_4: f32 = 0f;
    var p_19: vec3<f32>;
    var local_40: vec3<f32>;

    base_5 = base_4;
    x_55 = x_54;
    y_47 = y_46;
    z_55 = z_54;
    dx_21 = dx_20;
    dy_15 = dy_14;
    canopy_1 = canopy;
    let _e190: VoxelScene = u;
    ndl_2 = max(-(_e190.sunK.z), 0f);
    let _e199: i32 = x_55;
    let _e201: i32 = dx_21;
    let _e205: i32 = S;
    let _e209: i32 = y_47;
    let _e213: i32 = dy_15;
    let _e217: i32 = S;
    let _e221: i32 = z_55;
    p_19 = vec3<f32>((f32(_e199) + ((f32(_e201) + 0.5f) / f32(_e205))), (f32((_e209 + 1i)) - ((f32(_e213) + 0.5f) / f32(_e217))), f32(_e221));
    let _e225: f32 = ndl_2;
    let _e228: VoxelScene = u;
    if ((_e225 > 0f) && (_e228.sunK.y > 0f)) {
        {
            let _e234: vec3<f32> = p_19;
            let _e235: i32 = x_55;
            let _e236: i32 = y_47;
            let _e237: i32 = z_55;
            let _e241: f32 = sunReaches(_e234, vec3<i32>(_e235, _e236, (_e237 - 1i)));
            sun_4 = _e241;
        }
    }
    let _e242: vec3<f32> = p_19;
    let _e248: vec3<f32> = glowAt((_e242 - vec3<f32>(0f, 0f, 0.5f)));
    local_40 = _e248;
    let _e250: vec3<f32> = base_5;
    let _e251: i32 = x_55;
    let _e252: i32 = y_47;
    let _e253: i32 = z_55;
    let _e256: f32 = skyOpen(_e251, _e252, (_e253 - 1i));
    let _e257: i32 = x_55;
    let _e258: i32 = y_47;
    let _e259: i32 = z_55;
    let _e260: i32 = dx_21;
    let _e261: i32 = dy_15;
    let _e262: f32 = aoFront(_e257, _e258, _e259, _e260, _e261);
    let _e264: f32 = canopy_1;
    let _e266: f32 = sun_4;
    let _e267: f32 = ndl_2;
    let _e268: vec3<f32> = local_40;
    let _e269: vec3<f32> = litBy(_e250, ((_e256 * _e262) * _e264), _e266, _e267, _e268);
    return _e269;
}

fn shadeTop(base_6: vec3<f32>, x_56: i32, y_48: i32, z_56: i32, dx_22: i32, dy_16: i32, canopy_2: f32) -> vec3<f32> {
    var base_7: vec3<f32>;
    var x_57: i32;
    var y_49: i32;
    var z_57: i32;
    var dx_23: i32;
    var dy_17: i32;
    var canopy_3: f32;
    var ndl_3: f32;
    var sun_5: f32 = 0f;
    var p_20: vec3<f32>;
    var local_41: vec3<f32>;

    base_7 = base_6;
    x_57 = x_56;
    y_49 = y_48;
    z_57 = z_56;
    dx_23 = dx_22;
    dy_17 = dy_16;
    canopy_3 = canopy_2;
    let _e190: VoxelScene = u;
    ndl_3 = max(_e190.sunK.y, 0f);
    let _e198: i32 = x_57;
    let _e200: i32 = dx_23;
    let _e204: i32 = S;
    let _e208: i32 = y_49;
    let _e212: i32 = z_57;
    let _e216: i32 = dy_17;
    let _e220: i32 = RISE;
    p_20 = vec3<f32>((f32(_e198) + ((f32(_e200) + 0.5f) / f32(_e204))), f32((_e208 + 1i)), (f32((_e212 + 1i)) - ((f32(_e216) + 0.5f) / f32(_e220))));
    let _e226: f32 = ndl_3;
    if (_e226 > 0f) {
        {
            let _e229: vec3<f32> = p_20;
            let _e230: i32 = x_57;
            let _e231: i32 = y_49;
            let _e234: i32 = z_57;
            let _e236: f32 = sunReaches(_e229, vec3<i32>(_e230, (_e231 + 1i), _e234));
            sun_5 = _e236;
        }
    }
    let _e237: vec3<f32> = p_20;
    let _e243: vec3<f32> = glowAt((_e237 + vec3<f32>(0f, 0.5f, 0f)));
    local_41 = _e243;
    let _e245: vec3<f32> = base_7;
    let _e246: i32 = x_57;
    let _e247: i32 = y_49;
    let _e250: i32 = z_57;
    let _e251: f32 = skyOpen(_e246, (_e247 + 1i), _e250);
    let _e252: i32 = x_57;
    let _e253: i32 = y_49;
    let _e254: i32 = z_57;
    let _e255: i32 = dx_23;
    let _e256: i32 = dy_17;
    let _e257: f32 = aoTop(_e252, _e253, _e254, _e255, _e256);
    let _e259: f32 = canopy_3;
    let _e261: f32 = sun_5;
    let _e262: f32 = ndl_3;
    let _e263: vec3<f32> = local_41;
    let _e264: vec3<f32> = litBy(_e245, ((_e251 * _e257) * _e259), _e261, _e262, _e263);
    return _e264;
}

fn plantCellAt(c_19: vec4<u32>) -> vec3<i32> {
    var c_20: vec4<u32>;

    c_20 = c_19;
    let _e191: vec4<u32> = c_20;
    let _e196: vec4<u32> = c_20;
    let _e204: vec4<u32> = c_20;
    return vec3<i32>(i32((_e191.x & 4095u)), i32(((_e196.x >> 12u) & 1023u)), i32((_e204.x >> 22u)));
}

fn plantOf(c_21: vec4<u32>) -> u32 {
    var c_22: vec4<u32>;

    c_22 = c_21;
    let _e191: vec4<u32> = c_22;
    return (_e191.y & 524287u);
}

fn plantDying(c_23: vec4<u32>) -> bool {
    var c_24: vec4<u32>;

    c_24 = c_23;
    let _e191: vec4<u32> = c_24;
    return ((_e191.y & 524288u) != 0u);
}

fn plantWeight(c_25: vec4<u32>) -> f32 {
    var c_26: vec4<u32>;

    c_26 = c_25;
    let _e191: vec4<u32> = c_26;
    return (f32((_e191.y >> 20u)) / 4095f);
}

fn plantCarried(c_27: vec4<u32>) -> vec4<u32> {
    var c_28: vec4<u32>;

    c_28 = c_27;
    let _e191: vec4<u32> = c_28;
    let _e195: vec4<u32> = c_28;
    let _e202: vec4<u32> = c_28;
    let _e209: vec4<u32> = c_28;
    return vec4<u32>((_e191.w & 255u), ((_e195.w >> 8u) & 255u), ((_e202.w >> 16u) & 255u), (_e209.w >> 24u));
}

fn plantScale(c_29: vec4<u32>) -> f32 {
    var c_30: vec4<u32>;
    var since: f32;
    var local_42: f32;
    var local_43: f32;
    var r_6: f32;
    var s_1: f32;
    var local_44: f32;

    c_30 = c_29;
    let _e191: PlantPush = pc;
    let _e194: vec4<u32> = c_30;
    since = (_e191.t.x - bitcast<f32>(_e194.z));
    let _e199: PlantPush = pc;
    if (_e199.t.y > 0f) {
        let _e204: f32 = since;
        let _e205: PlantPush = pc;
        local_43 = clamp((_e204 / _e205.t.y), 0f, 1f);
    } else {
        let _e212: f32 = since;
        if (_e212 >= 0f) {
            local_42 = 1f;
        } else {
            local_42 = 0f;
        }
        let _e218: f32 = local_42;
        local_43 = _e218;
    }
    let _e220: f32 = local_43;
    r_6 = _e220;
    let _e222: f32 = r_6;
    let _e223: f32 = r_6;
    let _e227: f32 = r_6;
    s_1 = ((_e222 * _e223) * (3f - (2f * _e227)));
    let _e232: vec4<u32> = c_30;
    let _e233: bool = plantDying(_e232);
    if _e233 {
        let _e235: f32 = s_1;
        local_44 = (1f - _e235);
    } else {
        let _e237: f32 = s_1;
        local_44 = _e237;
    }
    let _e239: f32 = local_44;
    return _e239;
}

fn plantBox(c_31: vec4<u32>, ctr: ptr<function, vec3<f32>>, h_6: ptr<function, f32>) {
    var c_32: vec4<u32>;

    c_32 = c_31;
    let _e194: vec4<u32> = c_32;
    let _e195: f32 = plantScale(_e194);
    (*h_6) = (0.5f * _e195);
    let _e197: vec4<u32> = c_32;
    let _e198: vec3<i32> = plantCellAt(_e197);
    (*ctr) = (vec3<f32>(_e198) + vec3(0.5f));
    let _e204: vec3<f32> = (*ctr);
    let _e206: vec4<u32> = c_32;
    let _e207: u32 = plantOf(_e206);
    let _e209: f32 = global_2.tips[_e207];
    let _e210: vec4<u32> = c_32;
    let _e211: f32 = plantWeight(_e210);
    (*ctr).x = (_e204.x + (_e209 * _e211));
    return;
}

fn plantFaceZ(c_33: vec4<u32>, cap_1: bool, py: f32) -> f32 {
    var c_34: vec4<u32>;
    var cap_2: bool;
    var py_1: f32;
    var ctr_1: vec3<f32>;
    var h_7: f32;
    var zf_2: f32;
    var R: f32;
    var bottom: f32;

    c_34 = c_33;
    cap_2 = cap_1;
    py_1 = py;
    let _e197: vec4<u32> = c_34;
    plantBox(_e197, (&ctr_1), (&h_7));
    let _e202: vec3<f32> = ctr_1;
    let _e204: f32 = h_7;
    zf_2 = (_e202.z - _e204);
    let _e207: bool = cap_2;
    if !(_e207) {
        {
            let _e209: f32 = zf_2;
            return _e209;
        }
    }
    let _e210: PlantPush = pc;
    R = f32(_e210.geom.y);
    let _e215: PlantPush = pc;
    let _e219: vec3<f32> = ctr_1;
    let _e221: f32 = h_7;
    let _e223: PlantPush = pc;
    let _e229: f32 = zf_2;
    let _e230: f32 = R;
    bottom = ((f32(_e215.geom.z) - ((_e219.y + _e221) * f32(_e223.geom.x))) - (_e229 * _e230));
    let _e234: f32 = zf_2;
    let _e235: f32 = bottom;
    let _e236: f32 = py_1;
    let _e238: f32 = R;
    let _e242: f32 = h_7;
    return (_e234 + clamp(((_e235 - _e236) / _e238), 0f, (2f * _e242)));
}

fn plantDepth(z_58: f32) -> f32 {
    var z_59: f32;

    z_59 = z_58;
    let _e191: f32 = z_59;
    let _e194: PlantPush = pc;
    return clamp(((_e191 + 16f) / (f32(_e194.k.w) + 32f)), 0f, 1f);
}

fn plantZ(depth: f32) -> f32 {
    var depth_1: f32;

    depth_1 = depth;
    let _e191: f32 = depth_1;
    let _e192: PlantPush = pc;
    return ((_e191 * (f32(_e192.k.w) + 32f)) - 16f);
}

fn faceGrain(x_58: i32, y_50: i32, z_60: i32, dx_24: i32, dy_18: i32) -> f32 {
    var x_59: i32;
    var y_51: i32;
    var z_61: i32;
    var dx_25: i32;
    var dy_19: i32;
    var h_8: u32;

    x_59 = x_58;
    y_51 = y_50;
    z_61 = z_60;
    dx_25 = dx_24;
    dy_19 = dy_18;
    let _e201: i32 = x_59;
    let _e204: i32 = y_51;
    let _e208: i32 = z_61;
    let _e212: i32 = dx_25;
    let _e218: i32 = dy_19;
    h_8 = u32(((u32((((_e201 * 73856093i) ^ (_e204 * 19349663i)) ^ (_e208 * 83492791i))) ^ (u32(_e212) * 2654435761u)) ^ u32((_e218 * 38291i))));
    let _e225: u32 = h_8;
    let _e226: u32 = h_8;
    h_8 = ((_e225 ^ (_e226 >> 13u)) * 1274126177u);
    let _e233: u32 = h_8;
    return (f32((i32((_e233 & 15u)) - 7i)) * 0.005f);
}

fn blockFrontLit(x_60: i32, y_52: i32, z_62: i32, v_32: vec4<u32>, r_7: i32, dx_26: i32) -> vec3<f32> {
    var x_61: i32;
    var y_53: i32;
    var z_63: i32;
    var v_33: vec4<u32>;
    var r_8: i32;
    var dx_27: i32;
    var dy_20: i32;
    var openUp: bool;
    var openLeft: bool;
    var openRight: bool;
    var riser: bool;
    var onSide: bool;
    var m_9: i32;
    var local_45: i32;
    var slot_6: i32;
    var body_2: vec3<f32>;
    var own: vec3<f32>;
    var lit_2: vec3<f32>;
    var local_46: f32;
    var t_18: f32;
    var slope: vec3<f32>;
    var hz: f32;
    var local_47: vec3<f32>;
    var haze_2: f32;
    var c_35: vec3<f32>;
    var lit_3: vec3<f32>;
    var local_48: vec3<f32>;
    var local_49: vec3<f32>;

    x_61 = x_60;
    y_53 = y_52;
    z_63 = z_62;
    v_33 = v_32;
    r_8 = r_7;
    dx_27 = dx_26;
    let _e203: i32 = S;
    let _e206: i32 = r_8;
    dy_20 = ((_e203 - 1i) - _e206);
    let _e209: i32 = x_61;
    let _e210: i32 = y_53;
    let _e213: i32 = z_63;
    let _e214: bool = solidAt(_e209, (_e210 + 1i), _e213);
    openUp = !(_e214);
    let _e217: i32 = x_61;
    let _e220: i32 = y_53;
    let _e221: i32 = z_63;
    let _e222: bool = solidAt((_e217 - 1i), _e220, _e221);
    openLeft = !(_e222);
    let _e225: i32 = x_61;
    let _e228: i32 = y_53;
    let _e229: i32 = z_63;
    let _e230: bool = solidAt((_e225 + 1i), _e228, _e229);
    openRight = !(_e230);
    let _e233: bool = openUp;
    let _e234: i32 = z_63;
    let _e238: i32 = x_61;
    let _e239: i32 = y_53;
    let _e240: i32 = z_63;
    let _e243: bool = solidAt(_e238, _e239, (_e240 - 1i));
    let _e246: i32 = x_61;
    let _e247: i32 = y_53;
    let _e250: i32 = z_63;
    let _e253: bool = solidAt(_e246, (_e247 - 1i), (_e250 - 1i));
    riser = (((_e233 && (_e234 > 0i)) && !(_e243)) && _e253);
    let _e256: i32 = dx_27;
    let _e259: bool = openLeft;
    let _e261: i32 = dx_27;
    let _e264: i32 = S;
    let _e266: bool = openRight;
    onSide = (((_e256 == 0i) && _e259) || (((_e261 + 1i) == _e264) && _e266));
    let _e270: vec4<u32> = v_33;
    let _e271: i32 = matOf(_e270);
    m_9 = _e271;
    let _e273: i32 = m_9;
    let _e276: bool = openUp;
    let _e278: bool = texOn(TEX_TURF_SIDE);
    if (((_e273 == 3i) && _e276) && _e278) {
        local_45 = TEX_TURF_SIDE;
    } else {
        let _e281: i32 = m_9;
        local_45 = (2i * (_e281 - 1i));
    }
    let _e286: i32 = local_45;
    slot_6 = _e286;
    let _e288: vec4<u32> = v_33;
    let _e289: i32 = m_9;
    let _e290: i32 = slot_6;
    let _e291: i32 = x_61;
    let _e292: i32 = y_53;
    let _e293: i32 = z_63;
    let _e295: i32 = dx_27;
    let _e296: i32 = dy_20;
    let _e297: vec3<f32> = faceBase(_e289, _e290, _e291, _e292, _e293, 0i, _e295, _e296);
    let _e298: vec3<f32> = blockBody(_e288, _e297);
    body_2 = _e298;
    let _e300: i32 = m_9;
    let _e303: VoxelScene = u;
    if ((_e300 != 0i) && (_e303.roofK.z > 0f)) {
        {
            let _e309: vec3<f32> = body_2;
            let _e311: i32 = x_61;
            let _e312: i32 = y_53;
            let _e313: i32 = z_63;
            let _e314: i32 = dx_27;
            let _e315: i32 = dy_20;
            let _e316: f32 = faceGrain(_e311, _e312, _e313, _e314, _e315);
            let _e317: VoxelScene = u;
            body_2 = (_e309 * (1f + (_e316 * (_e317.roofK.z / 0.04f))));
        }
    }
    let _e325: vec3<f32> = body_2;
    let _e326: i32 = x_61;
    let _e327: i32 = y_53;
    let _e328: i32 = z_63;
    let _e329: i32 = dx_27;
    let _e330: i32 = dy_20;
    let _e331: i32 = x_61;
    let _e332: i32 = z_63;
    let _e335: i32 = y_53;
    let _e336: f32 = canopyAt(_e331, (_e332 - 1i), _e335);
    let _e337: vec3<f32> = shadeFront(_e325, _e326, _e327, _e328, _e329, _e330, _e336);
    own = _e337;
    let _e339: bool = riser;
    if _e339 {
        {
            let _e340: vec3<f32> = body_2;
            let _e341: i32 = x_61;
            let _e342: i32 = y_53;
            let _e343: i32 = z_63;
            let _e344: i32 = dx_27;
            let _e345: i32 = RISE;
            let _e348: i32 = x_61;
            let _e349: i32 = z_63;
            let _e350: i32 = y_53;
            let _e351: f32 = canopyAt(_e348, _e349, _e350);
            let _e352: vec3<f32> = shadeTop(_e340, _e341, _e342, _e343, _e344, (_e345 - 1i), _e351);
            lit_2 = _e352;
            let _e354: i32 = S;
            if (_e354 > 1i) {
                let _e357: i32 = dy_20;
                let _e359: i32 = S;
                local_46 = (f32(_e357) / f32((_e359 - 1i)));
            } else {
                local_46 = 0f;
            }
            let _e366: f32 = local_46;
            t_18 = _e366;
            let _e368: vec3<f32> = lit_2;
            let _e369: vec3<f32> = own;
            let _e370: VoxelScene = u;
            let _e373: f32 = t_18;
            slope = mix(_e368, _e369, vec3((_e370.shadeB.z * _e373)));
            let _e378: i32 = z_63;
            let _e380: f32 = t_18;
            let _e382: f32 = hazeAt((f32(_e378) - _e380));
            hz = _e382;
            let _e384: bool = onSide;
            if _e384 {
                let _e385: vec3<f32> = slope;
                let _e386: VoxelScene = u;
                local_47 = (_e385 * _e386.shadeB.x);
            } else {
                let _e390: vec3<f32> = slope;
                local_47 = _e390;
            }
            let _e392: vec3<f32> = local_47;
            let _e393: f32 = hz;
            let _e394: vec3<f32> = hazed(_e392, _e393);
            return _e394;
        }
    }
    let _e395: i32 = z_63;
    let _e397: f32 = hazeAt(f32(_e395));
    haze_2 = _e397;
    let _e400: i32 = dy_20;
    let _e403: bool = openUp;
    if ((_e400 == 0i) && _e403) {
        {
            let _e405: vec3<f32> = body_2;
            let _e406: i32 = x_61;
            let _e407: i32 = y_53;
            let _e408: i32 = z_63;
            let _e409: i32 = dx_27;
            let _e410: i32 = RISE;
            let _e413: i32 = x_61;
            let _e414: i32 = z_63;
            let _e415: i32 = y_53;
            let _e416: f32 = canopyAt(_e413, _e414, _e415);
            let _e417: vec3<f32> = shadeTop(_e405, _e406, _e407, _e408, _e409, (_e410 - 1i), _e416);
            lit_3 = _e417;
            let _e419: bool = onSide;
            if _e419 {
                let _e420: vec3<f32> = lit_3;
                local_48 = _e420;
            } else {
                let _e421: vec3<f32> = own;
                let _e422: vec3<f32> = lit_3;
                let _e423: VoxelScene = u;
                local_48 = mix(_e421, _e422, vec3(_e423.shadeA.w));
            }
            let _e429: vec3<f32> = local_48;
            c_35 = _e429;
        }
    } else {
        {
            let _e430: bool = onSide;
            if _e430 {
                let _e431: vec3<f32> = own;
                let _e432: VoxelScene = u;
                local_49 = (_e431 * _e432.shadeB.x);
            } else {
                let _e436: vec3<f32> = own;
                local_49 = _e436;
            }
            let _e438: vec3<f32> = local_49;
            c_35 = _e438;
        }
    }
    let _e439: vec3<f32> = c_35;
    let _e440: f32 = haze_2;
    let _e441: vec3<f32> = hazed(_e439, _e440);
    return _e441;
}

fn blockFront(x_62: i32, y_54: i32, z_64: i32, v_34: vec4<u32>, r_9: i32, dx_28: i32) -> vec3<f32> {
    var x_63: i32;
    var y_55: i32;
    var z_65: i32;
    var v_35: vec4<u32>;
    var r_10: i32;
    var dx_29: i32;
    var dy_21: i32;
    var openUp_1: bool;
    var openLeft_1: bool;
    var openRight_1: bool;
    var riser_1: bool;
    var onSide_1: bool;
    var m_10: i32;
    var local_50: i32;
    var slot_7: i32;
    var body_3: vec3<f32>;
    var lit_4: vec3<f32>;
    var local_51: f32;
    var t_19: f32;
    var slope_1: vec3<f32>;
    var hz_1: f32;
    var local_52: vec3<f32>;
    var haze_3: f32;
    var onCap: bool;
    var c_36: vec3<f32>;
    var local_53: vec3<f32>;
    var local_54: vec3<f32>;

    x_63 = x_62;
    y_55 = y_54;
    z_65 = z_64;
    v_35 = v_34;
    r_10 = r_9;
    dx_29 = dx_28;
    if LIT {
        {
            let _e203: i32 = x_63;
            let _e204: i32 = y_55;
            let _e205: i32 = z_65;
            let _e206: vec4<u32> = v_35;
            let _e207: i32 = r_10;
            let _e208: i32 = dx_29;
            let _e209: vec3<f32> = blockFrontLit(_e203, _e204, _e205, _e206, _e207, _e208);
            return _e209;
        }
    }
    let _e210: i32 = S;
    let _e213: i32 = r_10;
    dy_21 = ((_e210 - 1i) - _e213);
    let _e216: i32 = x_63;
    let _e217: i32 = y_55;
    let _e220: i32 = z_65;
    let _e221: bool = solidAt(_e216, (_e217 + 1i), _e220);
    openUp_1 = !(_e221);
    let _e224: i32 = x_63;
    let _e227: i32 = y_55;
    let _e228: i32 = z_65;
    let _e229: bool = solidAt((_e224 - 1i), _e227, _e228);
    openLeft_1 = !(_e229);
    let _e232: i32 = x_63;
    let _e235: i32 = y_55;
    let _e236: i32 = z_65;
    let _e237: bool = solidAt((_e232 + 1i), _e235, _e236);
    openRight_1 = !(_e237);
    let _e240: bool = openUp_1;
    let _e241: i32 = z_65;
    let _e245: i32 = x_63;
    let _e246: i32 = y_55;
    let _e247: i32 = z_65;
    let _e250: bool = solidAt(_e245, _e246, (_e247 - 1i));
    let _e253: i32 = x_63;
    let _e254: i32 = y_55;
    let _e257: i32 = z_65;
    let _e260: bool = solidAt(_e253, (_e254 - 1i), (_e257 - 1i));
    riser_1 = (((_e240 && (_e241 > 0i)) && !(_e250)) && _e260);
    let _e263: i32 = dx_29;
    let _e266: bool = openLeft_1;
    let _e268: i32 = dx_29;
    let _e271: i32 = S;
    let _e273: bool = openRight_1;
    onSide_1 = (((_e263 == 0i) && _e266) || (((_e268 + 1i) == _e271) && _e273));
    let _e277: vec4<u32> = v_35;
    let _e278: i32 = matOf(_e277);
    m_10 = _e278;
    let _e280: i32 = m_10;
    let _e283: bool = openUp_1;
    let _e285: bool = texOn(TEX_TURF_SIDE);
    if (((_e280 == 3i) && _e283) && _e285) {
        local_50 = TEX_TURF_SIDE;
    } else {
        let _e288: i32 = m_10;
        local_50 = (2i * (_e288 - 1i));
    }
    let _e293: i32 = local_50;
    slot_7 = _e293;
    let _e295: vec4<u32> = v_35;
    let _e296: i32 = m_10;
    let _e297: i32 = slot_7;
    let _e298: i32 = x_63;
    let _e299: i32 = y_55;
    let _e300: i32 = z_65;
    let _e302: i32 = dx_29;
    let _e303: i32 = dy_21;
    let _e304: vec3<f32> = faceBase(_e296, _e297, _e298, _e299, _e300, 0i, _e302, _e303);
    let _e305: vec3<f32> = blockBody(_e295, _e304);
    body_3 = _e305;
    let _e307: i32 = m_10;
    let _e310: VoxelScene = u;
    if ((_e307 != 0i) && (_e310.roofK.z > 0f)) {
        {
            let _e316: vec3<f32> = body_3;
            let _e318: i32 = x_63;
            let _e319: i32 = y_55;
            let _e320: i32 = z_65;
            let _e321: i32 = dx_29;
            let _e322: i32 = dy_21;
            let _e323: f32 = faceGrain(_e318, _e319, _e320, _e321, _e322);
            let _e324: VoxelScene = u;
            body_3 = (_e316 * (1f + (_e323 * (_e324.roofK.z / 0.04f))));
        }
    }
    let _e332: vec3<f32> = body_3;
    let _e333: i32 = x_63;
    let _e334: i32 = y_55;
    let _e335: i32 = z_65;
    let _e336: i32 = roofGap(_e333, _e334, _e335);
    let _e337: f32 = roofShade(_e336);
    let _e338: vec3<f32> = blockLit(_e332, _e337);
    lit_4 = _e338;
    let _e340: bool = riser_1;
    if _e340 {
        {
            let _e341: i32 = S;
            if (_e341 > 1i) {
                let _e344: i32 = dy_21;
                let _e346: i32 = S;
                local_51 = (f32(_e344) / f32((_e346 - 1i)));
            } else {
                local_51 = 0f;
            }
            let _e353: f32 = local_51;
            t_19 = _e353;
            let _e355: vec3<f32> = lit_4;
            let _e356: vec3<f32> = body_3;
            let _e357: VoxelScene = u;
            let _e360: f32 = t_19;
            slope_1 = mix(_e355, _e356, vec3((_e357.shadeB.z * _e360)));
            let _e365: i32 = z_65;
            let _e367: f32 = t_19;
            let _e369: f32 = hazeAt((f32(_e365) - _e367));
            hz_1 = _e369;
            let _e371: bool = onSide_1;
            if _e371 {
                let _e372: vec3<f32> = slope_1;
                let _e373: VoxelScene = u;
                local_52 = (_e372 * _e373.shadeB.x);
            } else {
                let _e377: vec3<f32> = slope_1;
                local_52 = _e377;
            }
            let _e379: vec3<f32> = local_52;
            let _e380: f32 = hz_1;
            let _e381: vec3<f32> = hazed(_e379, _e380);
            return _e381;
        }
    }
    let _e382: i32 = z_65;
    let _e384: f32 = hazeAt(f32(_e382));
    haze_3 = _e384;
    let _e386: i32 = dy_21;
    let _e389: bool = openUp_1;
    onCap = ((_e386 == 0i) && _e389);
    let _e393: bool = onCap;
    if _e393 {
        {
            let _e394: bool = onSide_1;
            if _e394 {
                let _e395: vec3<f32> = lit_4;
                local_53 = _e395;
            } else {
                let _e396: vec3<f32> = body_3;
                let _e397: vec3<f32> = lit_4;
                let _e398: VoxelScene = u;
                local_53 = mix(_e396, _e397, vec3(_e398.shadeA.w));
            }
            let _e404: vec3<f32> = local_53;
            c_36 = _e404;
        }
    } else {
        {
            let _e405: bool = onSide_1;
            if _e405 {
                let _e406: vec3<f32> = body_3;
                let _e407: VoxelScene = u;
                local_54 = (_e406 * _e407.shadeB.x);
            } else {
                let _e411: vec3<f32> = body_3;
                local_54 = _e411;
            }
            let _e413: vec3<f32> = local_54;
            c_36 = _e413;
        }
    }
    let _e414: vec3<f32> = c_36;
    let _e415: f32 = haze_3;
    let _e416: vec3<f32> = hazed(_e414, _e415);
    return _e416;
}

fn blockTopLit(x_64: i32, y_56: i32, z_66: i32, v_36: vec4<u32>, r_11: i32, dx_30: i32) -> vec3<f32> {
    var x_65: i32;
    var y_57: i32;
    var z_67: i32;
    var v_37: vec4<u32>;
    var r_12: i32;
    var dx_31: i32;
    var dy_22: i32;
    var openLeft_2: bool;
    var openRight_2: bool;
    var dropLeft: bool;
    var dropRight: bool;
    var backContinues: bool;
    var m_11: i32;
    var body_4: vec3<f32>;
    var own_1: vec3<f32>;
    var dark: vec3<f32>;
    var hz_2: f32;
    var local_55: vec3<f32>;
    var plane: vec3<f32>;
    var onDrop: bool;
    var local_56: vec3<f32>;

    x_65 = x_64;
    y_57 = y_56;
    z_67 = z_66;
    v_37 = v_36;
    r_12 = r_11;
    dx_31 = dx_30;
    let _e203: i32 = RISE;
    let _e206: i32 = r_12;
    dy_22 = ((_e203 - 1i) - _e206);
    let _e209: i32 = x_65;
    let _e212: i32 = y_57;
    let _e213: i32 = z_67;
    let _e214: bool = solidAt((_e209 - 1i), _e212, _e213);
    openLeft_2 = !(_e214);
    let _e217: i32 = x_65;
    let _e220: i32 = y_57;
    let _e221: i32 = z_67;
    let _e222: bool = solidAt((_e217 + 1i), _e220, _e221);
    openRight_2 = !(_e222);
    let _e225: bool = openLeft_2;
    let _e226: i32 = x_65;
    let _e229: i32 = y_57;
    let _e232: i32 = z_67;
    let _e233: bool = solidAt((_e226 - 1i), (_e229 - 1i), _e232);
    dropLeft = (_e225 && !(_e233));
    let _e237: bool = openRight_2;
    let _e238: i32 = x_65;
    let _e241: i32 = y_57;
    let _e244: i32 = z_67;
    let _e245: bool = solidAt((_e238 + 1i), (_e241 - 1i), _e244);
    dropRight = (_e237 && !(_e245));
    let _e249: i32 = z_67;
    let _e252: i32 = D;
    let _e254: i32 = x_65;
    let _e255: i32 = y_57;
    let _e256: i32 = z_67;
    let _e259: bool = solidAt(_e254, _e255, (_e256 + 1i));
    let _e261: i32 = x_65;
    let _e262: i32 = y_57;
    let _e265: i32 = z_67;
    let _e268: bool = solidAt(_e261, (_e262 + 1i), (_e265 + 1i));
    backContinues = ((((_e249 + 1i) < _e252) && _e259) && !(_e268));
    let _e272: vec4<u32> = v_37;
    let _e273: i32 = matOf(_e272);
    m_11 = _e273;
    let _e275: vec4<u32> = v_37;
    let _e276: i32 = m_11;
    let _e278: i32 = m_11;
    let _e284: i32 = x_65;
    let _e285: i32 = y_57;
    let _e286: i32 = z_67;
    let _e288: i32 = dx_31;
    let _e289: i32 = dy_22;
    let _e290: vec3<f32> = faceBase(_e276, ((2i * (_e278 - 1i)) + 1i), _e284, _e285, _e286, 1i, _e288, _e289);
    let _e291: vec3<f32> = blockBody(_e275, _e290);
    body_4 = _e291;
    let _e293: i32 = m_11;
    let _e296: VoxelScene = u;
    if ((_e293 != 0i) && (_e296.roofK.z > 0f)) {
        {
            let _e302: vec3<f32> = body_4;
            let _e304: i32 = x_65;
            let _e305: i32 = y_57;
            let _e306: i32 = z_67;
            let _e307: i32 = dx_31;
            let _e308: i32 = dy_22;
            let _e311: f32 = faceGrain(_e304, _e305, _e306, _e307, (_e308 + 100i));
            let _e312: VoxelScene = u;
            body_4 = (_e302 * (1f + (_e311 * (_e312.roofK.z / 0.04f))));
        }
    }
    let _e320: vec3<f32> = body_4;
    let _e321: i32 = x_65;
    let _e322: i32 = y_57;
    let _e323: i32 = z_67;
    let _e324: i32 = dx_31;
    let _e325: i32 = dy_22;
    let _e326: i32 = x_65;
    let _e327: i32 = z_67;
    let _e328: i32 = y_57;
    let _e329: f32 = canopyAt(_e326, _e327, _e328);
    let _e330: vec3<f32> = shadeTop(_e320, _e321, _e322, _e323, _e324, _e325, _e329);
    own_1 = _e330;
    let _e332: vec3<f32> = own_1;
    let _e333: VoxelScene = u;
    dark = (_e332 / vec3(_e333.shadeA.x));
    let _e339: i32 = z_67;
    let _e341: i32 = r_12;
    let _e343: i32 = RISE;
    let _e347: f32 = hazeAt((f32(_e339) + (f32(_e341) / f32(_e343))));
    hz_2 = _e347;
    let _e349: i32 = dy_22;
    let _e352: i32 = RISE;
    let _e356: bool = backContinues;
    if (((_e349 == 0i) && (_e352 > 2i)) && !(_e356)) {
        let _e359: vec3<f32> = own_1;
        let _e360: vec3<f32> = dark;
        let _e361: VoxelScene = u;
        local_55 = mix(_e359, _e360, vec3(_e361.shadeA.z));
    } else {
        let _e366: vec3<f32> = own_1;
        local_55 = _e366;
    }
    let _e368: vec3<f32> = local_55;
    plane = _e368;
    let _e370: i32 = dx_31;
    let _e373: bool = dropLeft;
    let _e375: i32 = dx_31;
    let _e378: i32 = S;
    let _e380: bool = dropRight;
    onDrop = (((_e370 == 0i) && _e373) || (((_e375 + 1i) == _e378) && _e380));
    let _e384: bool = onDrop;
    if _e384 {
        let _e385: vec3<f32> = plane;
        let _e386: vec3<f32> = dark;
        let _e387: VoxelScene = u;
        local_56 = mix(_e385, _e386, vec3(_e387.shadeB.y));
    } else {
        let _e392: vec3<f32> = plane;
        local_56 = _e392;
    }
    let _e394: vec3<f32> = local_56;
    let _e395: f32 = hz_2;
    let _e396: vec3<f32> = hazed(_e394, _e395);
    return _e396;
}

fn blockTop(x_66: i32, y_58: i32, z_68: i32, v_38: vec4<u32>, r_13: i32, dx_32: i32) -> vec3<f32> {
    var x_67: i32;
    var y_59: i32;
    var z_69: i32;
    var v_39: vec4<u32>;
    var r_14: i32;
    var dx_33: i32;
    var dy_23: i32;
    var openLeft_3: bool;
    var openRight_3: bool;
    var dropLeft_1: bool;
    var dropRight_1: bool;
    var backContinues_1: bool;
    var m_12: i32;
    var body_5: vec3<f32>;
    var lit_5: vec3<f32>;
    var hz_3: f32;
    var local_57: vec3<f32>;
    var plane_1: vec3<f32>;
    var onDrop_1: bool;
    var local_58: vec3<f32>;

    x_67 = x_66;
    y_59 = y_58;
    z_69 = z_68;
    v_39 = v_38;
    r_14 = r_13;
    dx_33 = dx_32;
    if LIT {
        {
            let _e203: i32 = x_67;
            let _e204: i32 = y_59;
            let _e205: i32 = z_69;
            let _e206: vec4<u32> = v_39;
            let _e207: i32 = r_14;
            let _e208: i32 = dx_33;
            let _e209: vec3<f32> = blockTopLit(_e203, _e204, _e205, _e206, _e207, _e208);
            return _e209;
        }
    }
    let _e210: i32 = RISE;
    let _e213: i32 = r_14;
    dy_23 = ((_e210 - 1i) - _e213);
    let _e216: i32 = x_67;
    let _e219: i32 = y_59;
    let _e220: i32 = z_69;
    let _e221: bool = solidAt((_e216 - 1i), _e219, _e220);
    openLeft_3 = !(_e221);
    let _e224: i32 = x_67;
    let _e227: i32 = y_59;
    let _e228: i32 = z_69;
    let _e229: bool = solidAt((_e224 + 1i), _e227, _e228);
    openRight_3 = !(_e229);
    let _e232: bool = openLeft_3;
    let _e233: i32 = x_67;
    let _e236: i32 = y_59;
    let _e239: i32 = z_69;
    let _e240: bool = solidAt((_e233 - 1i), (_e236 - 1i), _e239);
    dropLeft_1 = (_e232 && !(_e240));
    let _e244: bool = openRight_3;
    let _e245: i32 = x_67;
    let _e248: i32 = y_59;
    let _e251: i32 = z_69;
    let _e252: bool = solidAt((_e245 + 1i), (_e248 - 1i), _e251);
    dropRight_1 = (_e244 && !(_e252));
    let _e256: i32 = z_69;
    let _e259: i32 = D;
    let _e261: i32 = x_67;
    let _e262: i32 = y_59;
    let _e263: i32 = z_69;
    let _e266: bool = solidAt(_e261, _e262, (_e263 + 1i));
    let _e268: i32 = x_67;
    let _e269: i32 = y_59;
    let _e272: i32 = z_69;
    let _e275: bool = solidAt(_e268, (_e269 + 1i), (_e272 + 1i));
    backContinues_1 = ((((_e256 + 1i) < _e259) && _e266) && !(_e275));
    let _e279: vec4<u32> = v_39;
    let _e280: i32 = matOf(_e279);
    m_12 = _e280;
    let _e282: vec4<u32> = v_39;
    let _e283: i32 = m_12;
    let _e285: i32 = m_12;
    let _e291: i32 = x_67;
    let _e292: i32 = y_59;
    let _e293: i32 = z_69;
    let _e295: i32 = dx_33;
    let _e296: i32 = dy_23;
    let _e297: vec3<f32> = faceBase(_e283, ((2i * (_e285 - 1i)) + 1i), _e291, _e292, _e293, 1i, _e295, _e296);
    let _e298: vec3<f32> = blockBody(_e282, _e297);
    body_5 = _e298;
    let _e300: i32 = m_12;
    let _e303: VoxelScene = u;
    if ((_e300 != 0i) && (_e303.roofK.z > 0f)) {
        {
            let _e309: vec3<f32> = body_5;
            let _e311: i32 = x_67;
            let _e312: i32 = y_59;
            let _e313: i32 = z_69;
            let _e314: i32 = dx_33;
            let _e315: i32 = dy_23;
            let _e318: f32 = faceGrain(_e311, _e312, _e313, _e314, (_e315 + 100i));
            let _e319: VoxelScene = u;
            body_5 = (_e309 * (1f + (_e318 * (_e319.roofK.z / 0.04f))));
        }
    }
    let _e327: vec3<f32> = body_5;
    let _e328: i32 = x_67;
    let _e329: i32 = y_59;
    let _e330: i32 = z_69;
    let _e331: i32 = roofGap(_e328, _e329, _e330);
    let _e332: f32 = roofShade(_e331);
    let _e333: vec3<f32> = blockLit(_e327, _e332);
    lit_5 = _e333;
    let _e335: i32 = z_69;
    let _e337: i32 = r_14;
    let _e339: i32 = RISE;
    let _e343: f32 = hazeAt((f32(_e335) + (f32(_e337) / f32(_e339))));
    hz_3 = _e343;
    let _e345: i32 = dy_23;
    let _e348: i32 = RISE;
    let _e352: bool = backContinues_1;
    if (((_e345 == 0i) && (_e348 > 2i)) && !(_e352)) {
        let _e355: vec3<f32> = lit_5;
        let _e356: vec3<f32> = body_5;
        let _e357: VoxelScene = u;
        local_57 = mix(_e355, _e356, vec3(_e357.shadeA.z));
    } else {
        let _e362: vec3<f32> = lit_5;
        local_57 = _e362;
    }
    let _e364: vec3<f32> = local_57;
    plane_1 = _e364;
    let _e366: i32 = dx_33;
    let _e369: bool = dropLeft_1;
    let _e371: i32 = dx_33;
    let _e374: i32 = S;
    let _e376: bool = dropRight_1;
    onDrop_1 = (((_e366 == 0i) && _e369) || (((_e371 + 1i) == _e374) && _e376));
    let _e380: bool = onDrop_1;
    if _e380 {
        let _e381: vec3<f32> = plane_1;
        let _e382: vec3<f32> = body_5;
        let _e383: VoxelScene = u;
        local_58 = mix(_e381, _e382, vec3(_e383.shadeB.y));
    } else {
        let _e388: vec3<f32> = plane_1;
        local_58 = _e388;
    }
    let _e390: vec3<f32> = local_58;
    let _e391: f32 = hz_3;
    let _e392: vec3<f32> = hazed(_e390, _e391);
    return _e392;
}

fn glyphPigment(v_40: vec4<u32>, q_2: u32) -> vec3<f32> {
    var v_41: vec4<u32>;
    var q_3: u32;

    v_41 = v_40;
    q_3 = q_2;
    let _e195: vec4<u32> = v_41;
    let _e198: u32 = q_3;
    let _e202: vec3<f32> = styleAt(i32(_e195.w), i32((_e198 & 3u)));
    return _e202;
}

fn roleSlot(v_42: vec4<u32>, top_1: bool) -> i32 {
    var v_43: vec4<u32>;
    var top_2: bool;
    var role: i32;
    var local_59: i32;

    v_43 = v_42;
    top_2 = top_1;
    let _e196: vec4<u32> = v_43;
    let _e201: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e196.w)), 0i);
    role = i32((_e201.w + 0.5f));
    let _e207: i32 = role;
    let _e209: i32 = role;
    if ((_e207 < ROLE_BARK) || (_e209 > ROLE_DRAPE)) {
        {
            return -1i;
        }
    }
    let _e215: i32 = role;
    let _e219: bool = top_2;
    if _e219 {
        local_59 = 1i;
    } else {
        local_59 = 0i;
    }
    let _e223: i32 = local_59;
    return ((TEX_BARK_SIDE + (2i * (_e215 - ROLE_BARK))) + _e223);
}

fn speciesSlot(v_44: vec4<u32>, top_3: bool) -> i32 {
    var v_45: vec4<u32>;
    var top_4: bool;
    var local_60: i32;

    v_45 = v_44;
    top_4 = top_3;
    let _e195: bool = top_4;
    if _e195 {
        local_60 = 2i;
    } else {
        local_60 = 1i;
    }
    let _e199: i32 = local_60;
    let _e200: vec4<u32> = v_45;
    let _e205: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(_e199, i32(_e200.w)), 0i);
    return (i32((_e205.w + 0.5f)) - 1i);
}

fn cordShows(x_68: i32, y_60: i32, z_70: i32, top_5: bool, dx_34: i32, dy_24: i32) -> bool {
    var x_69: i32;
    var y_61: i32;
    var z_71: i32;
    var top_6: bool;
    var dx_35: i32;
    var dy_25: i32;

    x_69 = x_68;
    y_61 = y_60;
    z_71 = z_70;
    top_6 = top_5;
    dx_35 = dx_34;
    dy_25 = dy_24;
    let _e203: bool = top_6;
    return !(_e203);
}

fn plantTexel(x_70: i32, y_62: i32, z_72: i32, v_46: vec4<u32>, top_7: bool, dx_36: i32, dy_26: i32, base_8: ptr<function, vec3<f32>>) -> bool {
    var x_71: i32;
    var y_63: i32;
    var z_73: i32;
    var v_47: vec4<u32>;
    var top_8: bool;
    var dx_37: i32;
    var dy_27: i32;
    var own_2: i32;
    var local_61: i32;
    var t_20: vec4<f32>;
    var role_1: i32;
    var slot_8: i32;
    var local_62: i32;
    var t_21: vec4<f32>;

    x_71 = x_70;
    y_63 = y_62;
    z_73 = z_72;
    v_47 = v_46;
    top_8 = top_7;
    dx_37 = dx_36;
    dy_27 = dy_26;
    let _e206: vec4<u32> = v_47;
    let _e207: bool = cordCell(_e206);
    if _e207 {
        {
            let _e208: i32 = x_71;
            let _e209: i32 = y_63;
            let _e210: i32 = z_73;
            let _e211: bool = top_8;
            let _e212: i32 = dx_37;
            let _e213: i32 = dy_27;
            let _e214: bool = cordShows(_e208, _e209, _e210, _e211, _e212, _e213);
            return _e214;
        }
    }
    let _e215: vec4<u32> = v_47;
    let _e216: bool = top_8;
    let _e217: i32 = speciesSlot(_e215, _e216);
    own_2 = _e217;
    let _e219: i32 = own_2;
    if (_e219 >= 0i) {
        {
            let _e222: i32 = own_2;
            let _e223: i32 = x_71;
            let _e224: i32 = y_63;
            let _e225: i32 = z_73;
            let _e226: bool = top_8;
            if _e226 {
                local_61 = 1i;
            } else {
                local_61 = 0i;
            }
            let _e230: i32 = local_61;
            let _e231: i32 = dx_37;
            let _e232: i32 = dy_27;
            let _e233: vec4<f32> = faceTexel(_e222, _e223, _e224, _e225, _e230, _e231, _e232);
            t_20 = _e233;
            let _e236: vec4<u32> = v_47;
            let _e241: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e236.w)), 0i);
            role_1 = i32((_e241.w + 0.5f));
            let _e247: i32 = role_1;
            let _e252: i32 = role_1;
            let _e255: vec4<f32> = t_20;
            if (((_e247 >= 2i) && (_e252 <= ROLE_DRAPE)) && (_e255.w < 0.5f)) {
                {
                    return false;
                }
            }
            let _e261: vec4<f32> = t_20;
            let _e263: vec3<f32> = srgbToLinear(_e261.xyz);
            (*base_8) = _e263;
            return true;
        }
    }
    let _e265: vec4<u32> = v_47;
    let _e266: bool = top_8;
    let _e267: i32 = roleSlot(_e265, _e266);
    slot_8 = _e267;
    let _e269: i32 = slot_8;
    let _e270: bool = texOn(_e269);
    if !(_e270) {
        {
            return true;
        }
    }
    let _e273: i32 = slot_8;
    let _e274: i32 = x_71;
    let _e275: i32 = y_63;
    let _e276: i32 = z_73;
    let _e277: bool = top_8;
    if _e277 {
        local_62 = 1i;
    } else {
        local_62 = 0i;
    }
    let _e281: i32 = local_62;
    let _e282: i32 = dx_37;
    let _e283: i32 = dy_27;
    let _e284: vec4<f32> = faceTexel(_e273, _e274, _e275, _e276, _e281, _e282, _e283);
    t_21 = _e284;
    let _e286: i32 = slot_8;
    let _e291: vec4<f32> = t_21;
    if ((_e286 > 8i) && (_e291.w < 0.5f)) {
        {
            return false;
        }
    }
    let _e297: vec3<f32> = (*base_8);
    let _e298: vec4<f32> = t_21;
    (*base_8) = (_e297 * (_e298.x * 1.9921875f));
    return true;
}

fn glyphFrontAt(x_72: i32, y_64: i32, z_74: i32, v_48: vec4<u32>, row_5: i32, dx_38: i32, rgb: ptr<function, vec3<f32>>, glow: ptr<function, vec3<f32>>) -> bool {
    var x_73: i32;
    var y_65: i32;
    var z_75: i32;
    var v_49: vec4<u32>;
    var row_6: i32;
    var dx_39: i32;
    var q_4: u32;
    var pigment: vec4<f32>;
    var base_9: vec3<f32>;
    var tone: i32;
    var top_9: bool;
    var local_63: i32;
    var localY: i32;
    var local_64: f32;
    var hz_4: f32;
    var eAdd: vec3<f32> = vec3(0f);
    var keep: f32 = 1f;
    var e_5: vec3<f32>;
    var on_2: f32 = 1f;
    var coveredUp: bool;
    var animal: bool;
    var local_65: f32;
    var cf: f32;
    var local_66: f32;
    var ct: f32;
    var rim: bool;
    var tl: vec3<f32> = vec3(0f);
    var local_67: i32;
    var c_37: vec3<f32>;
    var gain: f32;
    var f: vec3<f32>;
    var local_68: vec3<f32>;
    var c_38: vec3<f32>;
    var gain_1: f32;
    var coveredUp_1: bool;

    x_73 = x_72;
    y_65 = y_64;
    z_75 = z_74;
    v_49 = v_48;
    row_6 = row_5;
    dx_39 = dx_38;
    (*glow) = vec3(0f);
    let _e207: vec4<u32> = v_49;
    let _e208: i32 = dx_39;
    let _e209: i32 = row_6;
    let _e210: u32 = cordGlyphTexel(_e207, _e208, _e209);
    q_4 = _e210;
    let _e212: u32 = q_4;
    let _e216: vec4<u32> = v_49;
    let _e221: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e212 & 3u)), i32(_e216.w)), 0i);
    pigment = _e221;
    let _e223: vec4<f32> = pigment;
    base_9 = _e223.xyz;
    let _e226: u32 = q_4;
    tone = i32((_e226 >> 2u));
    let _e232: i32 = row_6;
    let _e233: i32 = S;
    top_9 = (POSE && (_e232 >= _e233));
    let _e237: bool = top_9;
    if _e237 {
        let _e238: i32 = row_6;
        let _e239: i32 = S;
        local_63 = (_e238 - _e239);
    } else {
        let _e241: i32 = row_6;
        local_63 = _e241;
    }
    let _e243: i32 = local_63;
    localY = _e243;
    let _e245: i32 = z_75;
    let _e247: bool = top_9;
    if _e247 {
        let _e248: i32 = RISE;
        let _e251: i32 = localY;
        let _e254: i32 = RISE;
        local_64 = (f32(((_e248 - 1i) - _e251)) / f32(_e254));
    } else {
        local_64 = 0f;
    }
    let _e259: f32 = local_64;
    let _e261: f32 = hazeAt((f32(_e245) + _e259));
    hz_4 = _e261;
    let _e263: i32 = tone;
    let _e266: bool = top_9;
    if ((_e263 == 63i) && !(_e266)) {
        {
            return false;
        }
    }
    let _e275: i32 = tone;
    let _e277: vec4<f32> = pigment;
    let _e279: bool = emitsWhole(_e277.w);
    if (LIT && ((_e275 == TONE_EMIT) || _e279)) {
        {
            let _e282: vec4<u32> = v_49;
            let _e285: vec3<f32> = styleEmit(i32(_e282.w));
            e_5 = _e285;
            let _e289: i32 = x_73;
            let _e290: i32 = y_65;
            let _e291: i32 = z_75;
            let _e296: bool = cellEmit(_e289, _e290, _e291, (&e_5), (&on_2));
            let _e297: vec3<f32> = e_5;
            if any((_e297 > vec3(0f))) {
                {
                    let _e302: vec3<f32> = e_5;
                    let _e303: f32 = hz_4;
                    let _e304: vec3<f32> = hazed(_e302, _e303);
                    (*glow) = _e304;
                    let _e305: f32 = on_2;
                    if (_e305 >= 1f) {
                        {
                            let _e308: vec3<f32> = (*glow);
                            (*rgb) = _e308;
                            return true;
                        }
                    }
                    let _e310: vec3<f32> = e_5;
                    eAdd = _e310;
                    let _e312: f32 = on_2;
                    keep = (1f - _e312);
                }
            }
        }
    }
    let _e314: i32 = x_73;
    let _e315: i32 = y_65;
    let _e316: i32 = z_75;
    let _e317: vec4<u32> = v_49;
    let _e318: bool = top_9;
    let _e319: i32 = dx_39;
    let _e320: i32 = localY;
    let _e323: bool = plantTexel(_e314, _e315, _e316, _e317, _e318, _e319, _e320, (&base_9));
    if !(_e323) {
        {
            return false;
        }
    }
    if LIT {
        {
            let _e326: bool = top_9;
            let _e328: i32 = x_73;
            let _e329: i32 = y_65;
            let _e332: i32 = z_75;
            let _e333: bool = solidAt(_e328, (_e329 + 1i), _e332);
            let _e334: i32 = y_65;
            let _e337: bool = inY((_e334 + 1i));
            let _e338: i32 = x_73;
            let _e339: i32 = y_65;
            let _e342: i32 = z_75;
            let _e343: vec4<u32> = at(_e338, (_e339 + 1i), _e342);
            let _e344: i32 = partOf(_e343);
            let _e345: bool = isBlockPart(_e344);
            let _e347: i32 = x_73;
            let _e348: i32 = y_65;
            let _e351: i32 = z_75;
            let _e352: vec4<u32> = at(_e347, (_e348 + 1i), _e351);
            let _e353: bool = softMark(_e352);
            coveredUp = (!(_e326) && (_e333 || ((_e337 && _e345) && !(_e353))));
            let _e359: vec4<u32> = v_49;
            let _e360: i32 = partOf(_e359);
            animal = (_e360 == ANIMAL_INTERIM);
            let _e363: bool = animal;
            if _e363 {
                let _e364: i32 = x_73;
                let _e365: i32 = z_75;
                let _e368: i32 = y_65;
                let _e369: f32 = canopyAt(_e364, (_e365 - 1i), _e368);
                local_65 = _e369;
            } else {
                let _e370: vec4<u32> = v_49;
                let _e371: f32 = plantCanopy(_e370);
                local_65 = _e371;
            }
            let _e373: f32 = local_65;
            cf = _e373;
            let _e375: bool = animal;
            if _e375 {
                let _e376: i32 = x_73;
                let _e377: i32 = z_75;
                let _e378: i32 = y_65;
                let _e379: f32 = canopyAt(_e376, _e377, _e378);
                local_66 = _e379;
            } else {
                let _e380: vec4<u32> = v_49;
                let _e381: f32 = plantCanopy(_e380);
                local_66 = _e381;
            }
            let _e383: f32 = local_66;
            ct = _e383;
            let _e385: bool = top_9;
            let _e387: i32 = tone;
            let _e391: bool = coveredUp;
            rim = ((!(_e385) && (_e387 == 2i)) && !(_e391));
            let _e398: bool = top_9;
            let _e399: bool = rim;
            let _e401: i32 = tone;
            if ((_e398 || _e399) || (_e401 == 3i)) {
                {
                    let _e405: vec3<f32> = base_9;
                    let _e406: i32 = x_73;
                    let _e407: i32 = y_65;
                    let _e408: i32 = z_75;
                    let _e409: i32 = dx_39;
                    let _e410: bool = top_9;
                    if _e410 {
                        let _e411: i32 = localY;
                        local_67 = _e411;
                    } else {
                        let _e412: i32 = RISE;
                        local_67 = (_e412 - 1i);
                    }
                    let _e416: i32 = local_67;
                    let _e417: f32 = ct;
                    let _e418: vec3<f32> = shadeTop(_e405, _e406, _e407, _e408, _e409, _e416, _e417);
                    tl = _e418;
                }
            }
            let _e419: vec3<f32> = tl;
            c_37 = _e419;
            let _e421: bool = top_9;
            if _e421 {
                {
                    let _e422: i32 = tone;
                    if (_e422 == 1i) {
                        {
                            let _e425: vec3<f32> = c_37;
                            let _e426: VoxelScene = u;
                            c_37 = (_e425 * _e426.plantA.w);
                        }
                    } else {
                        let _e430: i32 = tone;
                        let _e433: i32 = tone;
                        if ((_e430 >= 32i) && (_e433 <= 47i)) {
                            {
                                let _e438: i32 = tone;
                                gain = (0.5f + (f32((_e438 & 15i)) / 16f));
                                let _e446: vec3<f32> = c_37;
                                let _e447: vec3<f32> = tl;
                                let _e448: VoxelScene = u;
                                let _e453: f32 = gain;
                                let _e455: VoxelScene = u;
                                c_37 = mix(_e446, ((_e447 / vec3(_e448.plantA.x)) * _e453), vec3(_e455.shadeB.y));
                            }
                        }
                    }
                }
            } else {
                let _e460: i32 = tone;
                if (_e460 != 3i) {
                    {
                        let _e463: i32 = tone;
                        if (_e463 == 1i) {
                            {
                                let _e466: vec3<f32> = base_9;
                                let _e467: VoxelScene = u;
                                base_9 = (_e466 * _e467.shadeB.x);
                            }
                        } else {
                            let _e471: i32 = tone;
                            let _e474: i32 = tone;
                            if ((_e471 == 4i) || (_e474 == 5i)) {
                                {
                                    let _e478: vec3<f32> = base_9;
                                    let _e479: vec4<u32> = v_49;
                                    let _e483: vec3<f32> = styleAt(i32(_e479.w), 0i);
                                    let _e484: VoxelScene = u;
                                    base_9 = mix(_e478, _e483, vec3(_e484.plantB.x));
                                    let _e489: i32 = tone;
                                    if (_e489 == 5i) {
                                        {
                                            let _e492: vec3<f32> = base_9;
                                            let _e494: VoxelScene = u;
                                            let _e497: i32 = x_73;
                                            let _e498: i32 = y_65;
                                            let _e499: i32 = z_75;
                                            let _e500: i32 = dx_39;
                                            let _e501: f32 = crownEdge(_e497, _e498, _e499, _e500);
                                            base_9 = (_e492 * mix(1f, _e494.plantA.w, _e501));
                                        }
                                    }
                                }
                            } else {
                                let _e504: i32 = tone;
                                let _e507: i32 = tone;
                                if ((_e504 >= 16i) && (_e507 <= 31i)) {
                                    {
                                        let _e511: vec3<f32> = base_9;
                                        let _e513: i32 = tone;
                                        base_9 = (_e511 * (0.5f + (f32((_e513 & 15i)) / 16f)));
                                    }
                                }
                            }
                        }
                        let _e521: vec3<f32> = base_9;
                        let _e522: i32 = x_73;
                        let _e523: i32 = y_65;
                        let _e524: i32 = z_75;
                        let _e525: i32 = dx_39;
                        let _e526: i32 = localY;
                        let _e527: f32 = cf;
                        let _e528: vec3<f32> = shadeFront(_e521, _e522, _e523, _e524, _e525, _e526, _e527);
                        f = _e528;
                        let _e530: bool = rim;
                        if _e530 {
                            let _e531: vec3<f32> = f;
                            let _e532: vec3<f32> = tl;
                            let _e533: VoxelScene = u;
                            local_68 = mix(_e531, _e532, vec3(_e533.plantA.z));
                        } else {
                            let _e538: vec3<f32> = f;
                            local_68 = _e538;
                        }
                        let _e540: vec3<f32> = local_68;
                        c_37 = _e540;
                    }
                }
            }
            let _e541: vec3<f32> = c_37;
            let _e542: f32 = keep;
            let _e544: vec3<f32> = eAdd;
            let _e546: f32 = hz_4;
            let _e547: vec3<f32> = hazed(((_e541 * _e542) + _e544), _e546);
            (*rgb) = _e547;
            return true;
        }
    }
    let _e549: bool = top_9;
    if _e549 {
        {
            let _e550: vec3<f32> = base_9;
            let _e551: i32 = x_73;
            let _e552: i32 = y_65;
            let _e553: i32 = z_75;
            let _e554: i32 = roofGap(_e551, _e552, _e553);
            let _e555: f32 = roofShade(_e554);
            let _e556: vec3<f32> = plantLit(_e550, _e555);
            c_38 = _e556;
            let _e558: i32 = tone;
            if (_e558 == 1i) {
                {
                    let _e561: vec3<f32> = c_38;
                    let _e562: VoxelScene = u;
                    c_38 = (_e561 * _e562.plantA.w);
                }
            } else {
                let _e566: i32 = tone;
                let _e569: i32 = tone;
                if ((_e566 >= 32i) && (_e569 <= 47i)) {
                    {
                        let _e574: i32 = tone;
                        gain_1 = (0.5f + (f32((_e574 & 15i)) / 16f));
                        let _e582: vec3<f32> = c_38;
                        let _e583: vec3<f32> = base_9;
                        let _e584: f32 = gain_1;
                        let _e586: VoxelScene = u;
                        c_38 = mix(_e582, (_e583 * _e584), vec3(_e586.shadeB.y));
                    }
                }
            }
            let _e591: vec3<f32> = c_38;
            let _e592: f32 = hz_4;
            let _e593: vec3<f32> = hazed(_e591, _e592);
            (*rgb) = _e593;
            return true;
        }
    }
    let _e595: i32 = x_73;
    let _e596: i32 = y_65;
    let _e599: i32 = z_75;
    let _e600: bool = solidAt(_e595, (_e596 + 1i), _e599);
    let _e601: i32 = y_65;
    let _e604: bool = inY((_e601 + 1i));
    let _e605: i32 = x_73;
    let _e606: i32 = y_65;
    let _e609: i32 = z_75;
    let _e610: vec4<u32> = at(_e605, (_e606 + 1i), _e609);
    let _e611: i32 = partOf(_e610);
    let _e612: bool = isBlockPart(_e611);
    let _e614: i32 = x_73;
    let _e615: i32 = y_65;
    let _e618: i32 = z_75;
    let _e619: vec4<u32> = at(_e614, (_e615 + 1i), _e618);
    let _e620: bool = softMark(_e619);
    coveredUp_1 = (_e600 || ((_e604 && _e612) && !(_e620)));
    let _e625: i32 = tone;
    if (_e625 == 1i) {
        {
            let _e628: vec3<f32> = base_9;
            let _e629: VoxelScene = u;
            base_9 = (_e628 * _e629.shadeB.x);
        }
    } else {
        let _e633: i32 = tone;
        let _e636: bool = coveredUp_1;
        if ((_e633 == 2i) && !(_e636)) {
            {
                let _e639: vec3<f32> = base_9;
                let _e640: vec3<f32> = base_9;
                let _e641: i32 = x_73;
                let _e642: i32 = y_65;
                let _e643: i32 = z_75;
                let _e644: i32 = roofGap(_e641, _e642, _e643);
                let _e645: f32 = roofShade(_e644);
                let _e646: vec3<f32> = plantLit(_e640, _e645);
                let _e647: VoxelScene = u;
                base_9 = mix(_e639, _e646, vec3(_e647.plantA.z));
            }
        } else {
            let _e652: i32 = tone;
            if (_e652 == 3i) {
                {
                    let _e655: vec3<f32> = base_9;
                    let _e656: i32 = x_73;
                    let _e657: i32 = y_65;
                    let _e658: i32 = z_75;
                    let _e659: i32 = roofGap(_e656, _e657, _e658);
                    let _e660: f32 = roofShade(_e659);
                    let _e661: vec3<f32> = plantLit(_e655, _e660);
                    base_9 = _e661;
                }
            } else {
                let _e662: i32 = tone;
                let _e665: i32 = tone;
                if ((_e662 == 4i) || (_e665 == 5i)) {
                    {
                        let _e669: vec3<f32> = base_9;
                        let _e670: vec4<u32> = v_49;
                        let _e674: vec3<f32> = styleAt(i32(_e670.w), 0i);
                        let _e675: VoxelScene = u;
                        base_9 = mix(_e669, _e674, vec3(_e675.plantB.x));
                        let _e680: i32 = tone;
                        if (_e680 == 5i) {
                            {
                                let _e683: vec3<f32> = base_9;
                                let _e685: VoxelScene = u;
                                let _e688: i32 = x_73;
                                let _e689: i32 = y_65;
                                let _e690: i32 = z_75;
                                let _e691: i32 = dx_39;
                                let _e692: f32 = crownEdge(_e688, _e689, _e690, _e691);
                                base_9 = (_e683 * mix(1f, _e685.plantA.w, _e692));
                            }
                        }
                    }
                } else {
                    let _e695: i32 = tone;
                    let _e698: i32 = tone;
                    if ((_e695 >= 16i) && (_e698 <= 31i)) {
                        {
                            let _e702: vec3<f32> = base_9;
                            let _e704: i32 = tone;
                            base_9 = (_e702 * (0.5f + (f32((_e704 & 15i)) / 16f)));
                        }
                    }
                }
            }
        }
    }
    let _e712: vec3<f32> = base_9;
    let _e713: f32 = hz_4;
    let _e714: vec3<f32> = hazed(_e712, _e713);
    (*rgb) = _e714;
    return true;
}

fn glyphCapAt(x_74: i32, y_66: i32, z_76: i32, v_50: vec4<u32>, localY_1: i32, dx_40: i32, rgb_1: ptr<function, vec3<f32>>, glow_1: ptr<function, vec3<f32>>) -> bool {
    var x_75: i32;
    var y_67: i32;
    var z_77: i32;
    var v_51: vec4<u32>;
    var localY_2: i32;
    var dx_41: i32;
    var r_15: i32;
    var atlasY: i32;
    var q_5: u32;
    var pigment_1: vec4<f32>;
    var base_10: vec3<f32>;
    var tone_1: i32;
    var eAdd_1: vec3<f32> = vec3(0f);
    var keep_1: f32 = 1f;
    var e_6: vec3<f32>;
    var on_3: f32 = 1f;
    var local_69: f32;
    var ct_1: f32;
    var own_3: vec3<f32>;
    var lc: vec3<f32>;
    var gain_2: f32;
    var shade_4: f32;
    var cap_3: vec3<f32>;
    var gain_3: f32;

    x_75 = x_74;
    y_67 = y_66;
    z_77 = z_76;
    v_51 = v_50;
    localY_2 = localY_1;
    dx_41 = dx_40;
    (*glow_1) = vec3(0f);
    let _e207: i32 = RISE;
    let _e210: i32 = localY_2;
    r_15 = ((_e207 - 1i) - _e210);
    let _e213: vec4<u32> = v_51;
    let _e214: i32 = partOf(_e213);
    let _e217: vec4<u32> = v_51;
    let _e218: i32 = glyphOf(_e217);
    let _e220: i32 = S;
    let _e221: i32 = RISE;
    let _e224: i32 = S;
    let _e226: i32 = localY_2;
    atlasY = (((((_e214 * 8i) + _e218) * (_e220 + _e221)) + _e224) + _e226);
    let _e229: i32 = dx_41;
    let _e230: i32 = atlasY;
    let _e233: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e229, _e230), 0i);
    q_5 = _e233.x;
    let _e236: u32 = q_5;
    let _e240: vec4<u32> = v_51;
    let _e245: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e236 & 3u)), i32(_e240.w)), 0i);
    pigment_1 = _e245;
    let _e247: vec4<f32> = pigment_1;
    base_10 = _e247.xyz;
    let _e250: u32 = q_5;
    tone_1 = i32((_e250 >> 2u));
    let _e261: i32 = tone_1;
    let _e263: vec4<f32> = pigment_1;
    let _e265: bool = emitsWhole(_e263.w);
    if (LIT && ((_e261 == TONE_EMIT) || _e265)) {
        {
            let _e268: vec4<u32> = v_51;
            let _e271: vec3<f32> = styleEmit(i32(_e268.w));
            e_6 = _e271;
            let _e275: i32 = x_75;
            let _e276: i32 = y_67;
            let _e277: i32 = z_77;
            let _e282: bool = cellEmit(_e275, _e276, _e277, (&e_6), (&on_3));
            let _e283: vec3<f32> = e_6;
            if any((_e283 > vec3(0f))) {
                {
                    let _e288: vec3<f32> = e_6;
                    let _e289: i32 = z_77;
                    let _e291: i32 = r_15;
                    let _e293: i32 = RISE;
                    let _e297: f32 = hazeAt((f32(_e289) + (f32(_e291) / f32(_e293))));
                    let _e298: vec3<f32> = hazed(_e288, _e297);
                    (*glow_1) = _e298;
                    let _e299: f32 = on_3;
                    if (_e299 >= 1f) {
                        {
                            let _e302: vec3<f32> = (*glow_1);
                            (*rgb_1) = _e302;
                            return true;
                        }
                    }
                    let _e304: vec3<f32> = e_6;
                    eAdd_1 = _e304;
                    let _e306: f32 = on_3;
                    keep_1 = (1f - _e306);
                }
            }
        }
    }
    let _e308: i32 = x_75;
    let _e309: i32 = y_67;
    let _e310: i32 = z_77;
    let _e311: vec4<u32> = v_51;
    let _e313: i32 = dx_41;
    let _e314: i32 = localY_2;
    let _e317: bool = plantTexel(_e308, _e309, _e310, _e311, true, _e313, _e314, (&base_10));
    if !(_e317) {
        {
            return false;
        }
    }
    if LIT {
        {
            let _e320: vec4<u32> = v_51;
            let _e321: i32 = partOf(_e320);
            if (_e321 == ANIMAL_INTERIM) {
                let _e323: i32 = x_75;
                let _e324: i32 = z_77;
                let _e325: i32 = y_67;
                let _e326: f32 = canopyAt(_e323, _e324, _e325);
                local_69 = _e326;
            } else {
                let _e327: vec4<u32> = v_51;
                let _e328: f32 = plantCanopy(_e327);
                local_69 = _e328;
            }
            let _e330: f32 = local_69;
            ct_1 = _e330;
            let _e332: vec3<f32> = base_10;
            let _e333: i32 = x_75;
            let _e334: i32 = y_67;
            let _e335: i32 = z_77;
            let _e336: i32 = dx_41;
            let _e337: i32 = localY_2;
            let _e338: f32 = ct_1;
            let _e339: vec3<f32> = shadeTop(_e332, _e333, _e334, _e335, _e336, _e337, _e338);
            own_3 = _e339;
            let _e341: vec3<f32> = own_3;
            lc = _e341;
            let _e343: i32 = tone_1;
            if (_e343 == 1i) {
                {
                    let _e346: vec3<f32> = lc;
                    let _e348: VoxelScene = u;
                    let _e351: i32 = x_75;
                    let _e352: i32 = y_67;
                    let _e353: i32 = z_77;
                    let _e354: i32 = dx_41;
                    let _e355: f32 = crownEdge(_e351, _e352, _e353, _e354);
                    lc = (_e346 * mix(1f, _e348.plantA.w, _e355));
                }
            } else {
                let _e358: i32 = tone_1;
                let _e361: i32 = tone_1;
                if ((_e358 >= 32i) && (_e361 <= 47i)) {
                    {
                        let _e366: i32 = tone_1;
                        gain_2 = (0.5f + (f32((_e366 & 15i)) / 16f));
                        let _e374: vec3<f32> = lc;
                        let _e375: vec3<f32> = own_3;
                        let _e376: VoxelScene = u;
                        let _e381: f32 = gain_2;
                        let _e383: VoxelScene = u;
                        lc = mix(_e374, ((_e375 / vec3(_e376.plantA.x)) * _e381), vec3(_e383.shadeB.y));
                    }
                }
            }
            let _e388: vec3<f32> = lc;
            let _e389: f32 = keep_1;
            let _e391: vec3<f32> = eAdd_1;
            let _e393: i32 = z_77;
            let _e395: i32 = r_15;
            let _e397: i32 = RISE;
            let _e401: f32 = hazeAt((f32(_e393) + (f32(_e395) / f32(_e397))));
            let _e402: vec3<f32> = hazed(((_e388 * _e389) + _e391), _e401);
            (*rgb_1) = _e402;
            return true;
        }
    }
    let _e404: i32 = x_75;
    let _e405: i32 = y_67;
    let _e406: i32 = z_77;
    let _e407: i32 = roofGap(_e404, _e405, _e406);
    let _e408: f32 = roofShade(_e407);
    shade_4 = _e408;
    let _e410: vec3<f32> = base_10;
    let _e411: f32 = shade_4;
    let _e412: vec3<f32> = plantLit(_e410, _e411);
    cap_3 = _e412;
    let _e414: i32 = tone_1;
    if (_e414 == 1i) {
        {
            let _e417: vec3<f32> = cap_3;
            let _e419: VoxelScene = u;
            let _e422: i32 = x_75;
            let _e423: i32 = y_67;
            let _e424: i32 = z_77;
            let _e425: i32 = dx_41;
            let _e426: f32 = crownEdge(_e422, _e423, _e424, _e425);
            cap_3 = (_e417 * mix(1f, _e419.plantA.w, _e426));
        }
    } else {
        let _e429: i32 = tone_1;
        let _e432: i32 = tone_1;
        if ((_e429 >= 32i) && (_e432 <= 47i)) {
            {
                let _e437: i32 = tone_1;
                gain_3 = (0.5f + (f32((_e437 & 15i)) / 16f));
                let _e445: vec3<f32> = cap_3;
                let _e446: vec3<f32> = base_10;
                let _e447: f32 = gain_3;
                let _e449: VoxelScene = u;
                cap_3 = mix(_e445, (_e446 * _e447), vec3(_e449.shadeB.y));
            }
        }
    }
    let _e454: vec3<f32> = cap_3;
    let _e455: i32 = z_77;
    let _e457: i32 = r_15;
    let _e459: i32 = RISE;
    let _e463: f32 = hazeAt((f32(_e455) + (f32(_e457) / f32(_e459))));
    let _e464: vec3<f32> = hazed(_e454, _e463);
    (*rgb_1) = _e464;
    return true;
}

fn plantHidden(v_52: vec4<u32>) -> bool {
    var v_53: vec4<u32>;

    v_53 = v_52;
    let _e193: vec4<u32> = v_53;
    let _e198: vec4<u32> = v_53;
    let _e199: i32 = partOf(_e198);
    let _e200: bool = plantBlockPart(_e199);
    return ((PLANT_MODELS && (_e193.y == 0u)) && _e200);
}

fn isPosed(v_54: vec4<u32>) -> bool {
    var v_55: vec4<u32>;

    v_55 = v_54;
    let _e193: vec4<u32> = v_55;
    let _e194: i32 = partOf(_e193);
    let _e197: vec4<u32> = v_55;
    return ((POSE && (_e194 == ANIMAL_INTERIM)) && (_e197.z != 0u));
}

fn posedSpan(c_39: vec4<u32>, sy: i32) -> vec2<i32> {
    var c_40: vec4<u32>;
    var sy_1: i32;
    var cls: i32;
    var db: vec2<i32>;
    var local_70: i32;

    c_40 = c_39;
    sy_1 = sy;
    let _e195: vec4<u32> = c_40;
    cls = i32((_e195.z >> 6u));
    let _e202: vec4<u32> = c_40;
    let _e209: vec4<f32> = u.poseK[i32((_e202.z & 63u))];
    db = vec2<i32>(_e209.xy);
    let _e213: i32 = cls;
    if (_e213 >= 2i) {
        {
            let _e216: i32 = sy_1;
            let _e217: i32 = S;
            let _e219: i32 = sy_1;
            let _e220: i32 = S;
            let _e222: i32 = sy_1;
            let _e225: i32 = S;
            let _e227: i32 = sy_1;
            let _e228: i32 = cls;
            let _e232: i32 = S;
            let _e234: vec2<i32> = db;
            return vec2<i32>((_e216 * _e217), max((_e219 * _e220), min(((_e222 + 1i) * _e225), ((((_e227 + _e228) - 1i) * _e232) - _e234.x))));
        }
    }
    let _e240: i32 = sy_1;
    let _e241: i32 = S;
    let _e243: vec2<i32> = db;
    let _e246: i32 = sy_1;
    let _e249: i32 = S;
    let _e251: vec2<i32> = db;
    let _e254: i32 = cls;
    if (_e254 == 1i) {
        let _e257: vec2<i32> = db;
        local_70 = _e257.y;
    } else {
        local_70 = 0i;
    }
    let _e261: i32 = local_70;
    return vec2<i32>(((_e240 * _e241) - _e243.x), ((((_e246 + 1i) * _e249) - _e251.x) - _e261));
}

fn posedRow(c_41: vec4<u32>, sy_2: i32, sp: vec2<i32>, q_6: i32) -> i32 {
    var c_42: vec4<u32>;
    var sy_3: i32;
    var sp_1: vec2<i32>;
    var q_7: i32;
    var t_22: i32;
    var local_71: i32;

    c_42 = c_41;
    sy_3 = sy_2;
    sp_1 = sp;
    q_7 = q_6;
    let _e199: vec4<u32> = c_42;
    if (i32((_e199.z >> 6u)) >= 2i) {
        {
            let _e207: i32 = sy_3;
            let _e210: i32 = S;
            let _e214: i32 = q_7;
            return ((((_e207 + 1i) * _e210) - 1i) - _e214);
        }
    }
    let _e216: vec2<i32> = sp_1;
    let _e220: i32 = q_7;
    t_22 = ((_e216.y - 1i) - _e220);
    let _e223: i32 = t_22;
    if (_e223 > 0i) {
        let _e226: i32 = t_22;
        let _e227: i32 = S;
        let _e229: vec2<i32> = sp_1;
        let _e231: vec2<i32> = sp_1;
        local_71 = ((_e226 + _e227) - (_e229.y - _e231.x));
    } else {
        local_71 = 0i;
    }
    let _e237: i32 = local_71;
    return _e237;
}

fn posedFind(x_76: i32, level: i32, z_78: i32, r_16: i32, gy: ptr<function, i32>, row_7: ptr<function, i32>) -> bool {
    var x_77: i32;
    var level_1: i32;
    var z_79: i32;
    var r_17: i32;
    var q_8: i32;
    var i_8: i32 = 0i;
    var sy_4: i32;
    var c_43: vec4<u32>;
    var sp_2: vec2<i32>;

    x_77 = x_76;
    level_1 = level;
    z_79 = z_78;
    r_17 = r_16;
    let _e201: i32 = level_1;
    let _e202: i32 = S;
    let _e204: i32 = r_17;
    q_8 = ((_e201 * _e202) + _e204);
    let _e207: i32 = level_1;
    (*gy) = _e207;
    (*row_7) = -1i;
    loop {
        let _e212: i32 = i_8;
        if !((_e212 < 3i)) {
            break;
        }
        {
            let _e219: i32 = level_1;
            let _e222: i32 = i_8;
            sy_4 = ((_e219 + 2i) - _e222);
            let _e225: i32 = sy_4;
            let _e226: bool = inY(_e225);
            if !(_e226) {
                {
                    continue;
                }
            }
            let _e228: i32 = x_77;
            let _e229: i32 = sy_4;
            let _e230: i32 = z_79;
            let _e231: vec4<u32> = at(_e228, _e229, _e230);
            c_43 = _e231;
            let _e233: vec4<u32> = c_43;
            let _e234: bool = isPosed(_e233);
            let _e236: vec4<u32> = c_43;
            let _e237: i32 = glyphOf(_e236);
            if (!(_e234) || (_e237 == 0i)) {
                {
                    continue;
                }
            }
            let _e241: vec4<u32> = c_43;
            let _e242: i32 = sy_4;
            let _e243: vec2<i32> = posedSpan(_e241, _e242);
            sp_2 = _e243;
            let _e245: vec2<i32> = sp_2;
            let _e247: vec2<i32> = sp_2;
            if (_e245.y <= _e247.x) {
                {
                    continue;
                }
            }
            let _e250: i32 = q_8;
            let _e251: vec2<i32> = sp_2;
            let _e254: i32 = q_8;
            let _e255: vec2<i32> = sp_2;
            if ((_e250 >= _e251.x) && (_e254 < _e255.y)) {
                {
                    let _e259: i32 = sy_4;
                    (*gy) = _e259;
                    let _e260: vec4<u32> = c_43;
                    let _e261: i32 = sy_4;
                    let _e262: vec2<i32> = sp_2;
                    let _e263: i32 = q_8;
                    let _e264: i32 = posedRow(_e260, _e261, _e262, _e263);
                    (*row_7) = _e264;
                    return true;
                }
            }
            let _e266: i32 = (*row_7);
            let _e269: i32 = q_8;
            let _e270: vec2<i32> = sp_2;
            let _e274: i32 = q_8;
            let _e275: vec2<i32> = sp_2;
            let _e277: i32 = RISE;
            if (((_e266 < 0i) && (_e269 >= _e270.y)) && (_e274 < (_e275.y + _e277))) {
                {
                    let _e281: i32 = sy_4;
                    (*gy) = _e281;
                    let _e282: i32 = S;
                    let _e283: i32 = RISE;
                    let _e287: i32 = q_8;
                    let _e288: vec2<i32> = sp_2;
                    (*row_7) = (((_e282 + _e283) - 1i) - (_e287 - _e288.y));
                }
            }
        }
        continuing {
            let _e216: i32 = i_8;
            i_8 = (_e216 + 1i);
        }
    }
    let _e292: i32 = (*row_7);
    return (_e292 >= 0i);
}

fn posedCapRow(v_56: vec4<u32>, level_2: i32, r_18: i32) -> i32 {
    var v_57: vec4<u32>;
    var level_3: i32;
    var r_19: i32;
    var sp_3: vec2<i32>;
    var q_9: i32;
    var local_72: i32;

    v_57 = v_56;
    level_3 = level_2;
    r_19 = r_18;
    let _e197: vec4<u32> = v_57;
    let _e198: i32 = glyphOf(_e197);
    if (_e198 == 0i) {
        {
            return -1i;
        }
    }
    let _e203: vec4<u32> = v_57;
    let _e204: i32 = level_3;
    let _e207: vec2<i32> = posedSpan(_e203, (_e204 - 1i));
    sp_3 = _e207;
    let _e209: vec2<i32> = sp_3;
    let _e211: vec2<i32> = sp_3;
    if (_e209.y <= _e211.x) {
        {
            return -1i;
        }
    }
    let _e216: i32 = level_3;
    let _e217: i32 = S;
    let _e219: i32 = r_19;
    q_9 = ((_e216 * _e217) + _e219);
    let _e222: i32 = q_9;
    let _e223: vec2<i32> = sp_3;
    let _e226: i32 = q_9;
    let _e227: vec2<i32> = sp_3;
    let _e229: i32 = RISE;
    if ((_e222 >= _e223.y) && (_e226 < (_e227.y + _e229))) {
        let _e233: i32 = RISE;
        let _e236: i32 = q_9;
        let _e237: vec2<i32> = sp_3;
        local_72 = ((_e233 - 1i) - (_e236 - _e237.y));
    } else {
        local_72 = -1i;
    }
    let _e244: i32 = local_72;
    return _e244;
}

fn nearerOwns(x_78: i32, y_68: i32, z_80: i32, row_8: i32) -> bool {
    var x_79: i32;
    var y_69: i32;
    var z_81: i32;
    var row_9: i32;
    var R_1: i32;
    var dy_28: i32 = 0i;
    var yn: i32;
    var f_1: i32;
    var top_10: i32;
    var fill: i32;
    var skin: i32;
    var local_73: i32;

    x_79 = x_78;
    y_69 = y_68;
    z_81 = z_80;
    row_9 = row_8;
    let _e199: i32 = z_81;
    if (_e199 == 0i) {
        {
            return false;
        }
    }
    let _e203: i32 = y_69;
    let _e204: i32 = z_81;
    let _e205: i32 = frontRow(_e203, _e204);
    R_1 = _e205;
    loop {
        let _e209: i32 = dy_28;
        if !((_e209 < 3i)) {
            break;
        }
        {
            let _e216: i32 = y_69;
            let _e217: i32 = dy_28;
            yn = (_e216 + _e217);
            let _e220: i32 = R_1;
            let _e221: i32 = RISE;
            let _e223: i32 = dy_28;
            let _e224: i32 = S;
            f_1 = ((_e220 + _e221) - (_e223 * _e224));
            let _e229: i32 = x_79;
            let _e230: i32 = yn;
            let _e231: i32 = z_81;
            let _e234: bool = solidAt(_e229, _e230, (_e231 - 1i));
            if _e234 {
                {
                    let _e235: i32 = f_1;
                    let _e236: i32 = RISE;
                    top_10 = (_e235 - _e236);
                }
            } else {
                {
                    let _e238: i32 = x_79;
                    let _e239: i32 = yn;
                    let _e240: i32 = z_81;
                    let _e243: i32 = fillPxAt(_e238, _e239, (_e240 - 1i));
                    fill = _e243;
                    let _e245: i32 = fill;
                    if (_e245 == 0i) {
                        {
                            continue;
                        }
                    }
                    let _e248: i32 = f_1;
                    let _e249: i32 = S;
                    let _e251: i32 = fill;
                    skin = ((_e248 + _e249) - _e251);
                    let _e254: i32 = x_79;
                    let _e255: i32 = yn;
                    let _e256: i32 = z_81;
                    let _e259: bool = waterOpenUp(_e254, _e255, (_e256 - 1i));
                    if _e259 {
                        let _e260: i32 = skin;
                        let _e261: i32 = RISE;
                        local_73 = (_e260 - _e261);
                    } else {
                        let _e263: i32 = skin;
                        local_73 = _e263;
                    }
                    let _e265: i32 = local_73;
                    top_10 = _e265;
                }
            }
            let _e266: i32 = row_9;
            let _e267: i32 = top_10;
            let _e269: i32 = row_9;
            let _e270: i32 = f_1;
            let _e271: i32 = S;
            if ((_e266 >= _e267) && (_e269 < (_e270 + _e271))) {
                {
                    return true;
                }
            }
        }
        continuing {
            let _e213: i32 = dy_28;
            dy_28 = (_e213 + 1i);
        }
    }
    return false;
}

fn waterLitReset() {
    wEntry = -1i;
    wScatter = vec3(0f);
    wHaze = 0f;
    wSurf = 0f;
    return;
}

fn waterSigma() -> vec3<f32> {
    let _e197: VoxelScene = u;
    let _e207: VoxelScene = u;
    return (-(log(max(_e197.waterDeepC.xyz, vec3(0.00024414063f)))) * _e207.waterL.x);
}

fn fallingAt(x_80: i32, y_70: i32, z_82: i32) -> bool {
    var x_81: i32;
    var y_71: i32;
    var z_83: i32;

    x_81 = x_80;
    y_71 = y_70;
    z_83 = z_82;
    let _e205: i32 = y_71;
    let _e208: i32 = x_81;
    let _e209: i32 = y_71;
    let _e212: i32 = z_83;
    let _e213: bool = solidAt(_e208, (_e209 - 1i), _e212);
    let _e216: i32 = x_81;
    let _e217: i32 = y_71;
    let _e220: i32 = z_83;
    let _e221: i32 = fillPxAt(_e216, (_e217 - 1i), _e220);
    let _e222: i32 = S;
    return (((_e205 > 0i) && !(_e213)) && (_e221 < _e222));
}

fn waterAtLit(x_82: i32, y_72: i32, z_84: i32, v_58: vec4<u32>, row_10: i32, acc: ptr<function, vec3<f32>>, trans_2: ptr<function, vec3<f32>>) {
    var x_83: i32;
    var y_73: i32;
    var z_85: i32;
    var v_59: vec4<u32>;
    var row_11: i32;
    var fill_1: i32;
    var R_2: i32;
    var bottom_1: i32;
    var skinRow: i32;
    var openUp_2: bool;
    var topRow: bool;
    var nearSolid: bool;
    var local_74: i32;
    var nearFill: i32;
    var nearSkin: i32;
    var stop: i32;
    var nearOpenUp: bool;
    var local_75: i32;
    var bodyRow: bool;
    var k_7: f32;
    var yf_2: f32;
    var h_9: f32;
    var local_76: i32;
    var falling: bool;
    var local_77: i32;
    var ys: i32;
    var i_9: i32 = 0i;
    var local_78: i32;
    var a_11: f32;
    var alpha: f32;
    var skinAlpha: f32;
    var a_12: f32;
    var c_44: vec3<f32>;
    var hf: f32;
    var isSkin: bool;
    var local_79: f32;
    var local_80: vec3<f32>;
    var top_11: f32;
    var hi_4: f32;
    var lo_5: f32;
    var l_5: f32;
    var T: vec3<f32>;
    var depth_2: f32;
    var cin: vec3<f32>;

    x_83 = x_82;
    y_73 = y_72;
    z_85 = z_84;
    v_59 = v_58;
    row_11 = row_10;
    let _e211: vec4<u32> = v_59;
    let _e214: i32 = fillPxQ(i32(_e211.y));
    fill_1 = _e214;
    let _e216: i32 = y_73;
    let _e217: i32 = z_85;
    let _e218: i32 = frontRow(_e216, _e217);
    R_2 = _e218;
    let _e220: i32 = R_2;
    let _e221: i32 = S;
    bottom_1 = (_e220 + _e221);
    let _e224: i32 = bottom_1;
    let _e225: i32 = fill_1;
    skinRow = (_e224 - _e225);
    let _e228: i32 = x_83;
    let _e229: i32 = y_73;
    let _e230: i32 = z_85;
    let _e231: bool = waterOpenUp(_e228, _e229, _e230);
    openUp_2 = _e231;
    let _e233: bool = openUp_2;
    let _e234: i32 = row_11;
    let _e235: i32 = skinRow;
    let _e236: i32 = RISE;
    let _e240: i32 = row_11;
    let _e241: i32 = skinRow;
    let _e244: i32 = x_83;
    let _e245: i32 = y_73;
    let _e246: i32 = z_85;
    let _e247: i32 = row_11;
    let _e248: bool = nearerOwns(_e244, _e245, _e246, _e247);
    topRow = (((_e233 && (_e234 >= (_e235 - _e236))) && (_e240 < _e241)) && !(_e248));
    let _e252: i32 = z_85;
    let _e255: i32 = x_83;
    let _e256: i32 = y_73;
    let _e257: i32 = z_85;
    let _e260: bool = solidAt(_e255, _e256, (_e257 - 1i));
    nearSolid = ((_e252 > 0i) && _e260);
    let _e263: i32 = z_85;
    if (_e263 > 0i) {
        let _e266: i32 = x_83;
        let _e267: i32 = y_73;
        let _e268: i32 = z_85;
        let _e271: i32 = fillPxAt(_e266, _e267, (_e268 - 1i));
        local_74 = _e271;
    } else {
        local_74 = 0i;
    }
    let _e274: i32 = local_74;
    nearFill = _e274;
    let _e276: i32 = R_2;
    let _e277: i32 = RISE;
    let _e279: i32 = S;
    let _e281: i32 = nearFill;
    nearSkin = (((_e276 + _e277) + _e279) - _e281);
    let _e284: i32 = bottom_1;
    stop = _e284;
    let _e286: bool = nearSolid;
    if _e286 {
        {
            let _e287: i32 = skinRow;
            stop = _e287;
        }
    } else {
        let _e288: i32 = nearFill;
        if (_e288 > 0i) {
            {
                let _e291: i32 = x_83;
                let _e292: i32 = y_73;
                let _e293: i32 = z_85;
                let _e296: bool = waterOpenUp(_e291, _e292, (_e293 - 1i));
                nearOpenUp = _e296;
                let _e298: i32 = bottom_1;
                let _e299: bool = nearOpenUp;
                if _e299 {
                    let _e300: i32 = nearSkin;
                    let _e301: i32 = RISE;
                    local_75 = (_e300 - _e301);
                } else {
                    let _e303: i32 = nearSkin;
                    local_75 = _e303;
                }
                let _e305: i32 = local_75;
                stop = min(_e298, _e305);
            }
        }
    }
    let _e307: i32 = row_11;
    let _e308: i32 = skinRow;
    let _e310: i32 = row_11;
    let _e311: i32 = stop;
    bodyRow = ((_e307 >= _e308) && (_e310 < _e311));
    let _e315: i32 = RISE;
    let _e317: i32 = S;
    k_7 = (f32(_e315) / f32(_e317));
    let _e321: i32 = BASE;
    let _e322: i32 = z_85;
    let _e323: i32 = RISE;
    let _e326: i32 = row_11;
    let _e333: i32 = S;
    yf_2 = ((f32((((_e321 - (_e322 * _e323)) - _e326) - 1i)) + 0.5f) / f32(_e333));
    let _e337: i32 = z_85;
    let _e341: f32 = hazeAt((f32(_e337) + 0.5f));
    h_9 = _e341;
    let _e343: i32 = wEntry;
    if (_e343 < 0i) {
        {
            let _e346: bool = topRow;
            let _e348: bool = bodyRow;
            if (!(_e346) && !(_e348)) {
                {
                    return;
                }
            }
            let _e351: i32 = y_73;
            let _e352: i32 = z_85;
            let _e357: bool = topRow;
            if _e357 {
                local_76 = 16777216i;
            } else {
                local_76 = 0i;
            }
            let _e364: i32 = local_76;
            wEntry = ((_e351 | (_e352 << 12u)) | _e364);
            let _e366: i32 = x_83;
            let _e367: i32 = y_73;
            let _e368: i32 = z_85;
            let _e369: bool = fallingAt(_e366, _e367, _e368);
            falling = _e369;
            let _e371: i32 = z_85;
            let _e374: bool = topRow;
            let _e377: bool = falling;
            if (((_e371 == 0i) && !(_e374)) || _e377) {
                {
                    let _e379: i32 = wEntry;
                    let _e380: bool = falling;
                    if _e380 {
                        local_77 = ENTRY_FALLING;
                    } else {
                        local_77 = 0i;
                    }
                    let _e383: i32 = local_77;
                    wEntry = (_e379 | (ENTRY_FLAT | _e383));
                }
            } else {
                {
                    let _e386: i32 = y_73;
                    ys = _e386;
                    loop {
                        let _e390: i32 = i_9;
                        let _e393: bool = topRow;
                        if !(((_e390 < 8i) && !(_e393))) {
                            break;
                        }
                        {
                            let _e400: i32 = x_83;
                            let _e401: i32 = ys;
                            let _e404: i32 = z_85;
                            let _e405: bool = solidAt(_e400, (_e401 + 1i), _e404);
                            let _e406: i32 = x_83;
                            let _e407: i32 = ys;
                            let _e410: i32 = z_85;
                            let _e411: i32 = freeQ(_e406, (_e407 + 1i), _e410);
                            if (_e405 || (_e411 == 0i)) {
                                {
                                    break;
                                }
                            }
                            let _e415: i32 = ys;
                            ys = (_e415 + 1i);
                        }
                        continuing {
                            let _e397: i32 = i_9;
                            i_9 = (_e397 + 1i);
                        }
                    }
                    let _e418: i32 = ys;
                    let _e420: bool = topRow;
                    if _e420 {
                        let _e421: i32 = fill_1;
                        local_78 = _e421;
                    } else {
                        let _e422: i32 = x_83;
                        let _e423: i32 = ys;
                        let _e424: i32 = z_85;
                        let _e425: i32 = fillPxAt(_e422, _e423, _e424);
                        local_78 = _e425;
                    }
                    let _e427: i32 = local_78;
                    let _e429: i32 = S;
                    wSurf = (f32(_e418) + (f32(_e427) / f32(_e429)));
                    let _e433: bool = bodyRow;
                    let _e434: i32 = row_11;
                    let _e435: i32 = skinRow;
                    let _e438: bool = openUp_2;
                    if ((_e433 && (_e434 == _e435)) && _e438) {
                        {
                            let _e440: VoxelScene = u;
                            let _e446: VoxelScene = u;
                            a_11 = min((clamp(_e440.knobs.y, 0f, 1f) * _e446.waterK.x), 0.95f);
                            let _e453: vec3<f32> = wScatter;
                            let _e454: vec3<f32> = (*trans_2);
                            let _e455: f32 = a_11;
                            let _e458: f32 = h_9;
                            let _e461: VoxelScene = u;
                            wScatter = (_e453 + (((_e454 * _e455) * (1f - _e458)) * _e461.waterSurfaceC.xyz));
                            let _e466: f32 = wHaze;
                            let _e467: vec3<f32> = (*trans_2);
                            let _e468: f32 = a_11;
                            let _e469: f32 = h_9;
                            wHaze = (_e466 + dot(_e467, vec3(((_e468 * _e469) / 3f))));
                            let _e476: vec3<f32> = (*trans_2);
                            let _e478: f32 = a_11;
                            (*trans_2) = (_e476 * (1f - _e478));
                        }
                    }
                }
            }
        }
    }
    let _e481: i32 = wEntry;
    if ((_e481 & ENTRY_FLAT) != 0i) {
        {
            let _e485: VoxelScene = u;
            alpha = clamp(_e485.knobs.y, 0f, 1f);
            let _e492: f32 = alpha;
            let _e493: VoxelScene = u;
            skinAlpha = min((_e492 * _e493.waterK.x), 0.95f);
            let _e503: bool = topRow;
            if _e503 {
                {
                    let _e504: f32 = skinAlpha;
                    let _e505: VoxelScene = u;
                    a_12 = (_e504 * _e505.waterK.y);
                    let _e509: VoxelScene = u;
                    c_44 = _e509.waterSurfaceC.xyz;
                    let _e512: i32 = z_85;
                    let _e514: i32 = RISE;
                    let _e517: i32 = row_11;
                    let _e518: i32 = skinRow;
                    let _e519: i32 = RISE;
                    let _e524: i32 = RISE;
                    let _e528: f32 = hazeAt((f32(_e512) + (f32(((_e514 - 1i) - (_e517 - (_e518 - _e519)))) / f32(_e524))));
                    hf = _e528;
                }
            } else {
                let _e529: bool = bodyRow;
                if _e529 {
                    {
                        let _e530: i32 = row_11;
                        let _e531: i32 = skinRow;
                        let _e533: bool = openUp_2;
                        isSkin = ((_e530 == _e531) && _e533);
                        let _e536: bool = isSkin;
                        if _e536 {
                            let _e537: f32 = skinAlpha;
                            local_79 = _e537;
                        } else {
                            let _e538: f32 = alpha;
                            local_79 = _e538;
                        }
                        let _e540: f32 = local_79;
                        a_12 = _e540;
                        let _e541: bool = isSkin;
                        if _e541 {
                            let _e542: VoxelScene = u;
                            local_80 = _e542.waterSurfaceC.xyz;
                        } else {
                            let _e545: VoxelScene = u;
                            local_80 = _e545.waterDeepC.xyz;
                        }
                        let _e549: vec3<f32> = local_80;
                        c_44 = _e549;
                        let _e550: i32 = z_85;
                        let _e552: f32 = hazeAt(f32(_e550));
                        hf = _e552;
                    }
                } else {
                    {
                        return;
                    }
                }
            }
            let _e553: vec3<f32> = wScatter;
            let _e554: vec3<f32> = (*trans_2);
            let _e555: f32 = a_12;
            let _e558: f32 = hf;
            let _e561: vec3<f32> = c_44;
            wScatter = (_e553 + (((_e554 * _e555) * (1f - _e558)) * _e561));
            let _e564: f32 = wHaze;
            let _e565: vec3<f32> = (*trans_2);
            let _e566: f32 = a_12;
            let _e567: f32 = hf;
            wHaze = (_e564 + dot(_e565, vec3(((_e566 * _e567) / 3f))));
            let _e574: vec3<f32> = (*trans_2);
            let _e576: f32 = a_12;
            (*trans_2) = (_e574 * (1f - _e576));
            return;
        }
    }
    let _e579: i32 = y_73;
    let _e581: i32 = fill_1;
    let _e583: i32 = S;
    top_11 = (f32(_e579) + (f32(_e581) / f32(_e583)));
    let _e588: f32 = yf_2;
    let _e589: f32 = top_11;
    hi_4 = min(_e588, _e589);
    let _e592: f32 = yf_2;
    let _e593: f32 = k_7;
    let _e595: i32 = y_73;
    lo_5 = max((_e592 - _e593), f32(_e595));
    let _e599: f32 = hi_4;
    let _e600: f32 = lo_5;
    let _e604: f32 = k_7;
    let _e607: f32 = k_7;
    let _e608: f32 = k_7;
    l_5 = ((max((_e599 - _e600), 0f) / _e604) * sqrt((1f + (_e607 * _e608))));
    let _e614: f32 = l_5;
    if (_e614 <= 0f) {
        {
            return;
        }
    }
    let _e617: vec3<f32> = waterSigma();
    let _e619: f32 = l_5;
    T = exp((-(_e617) * _e619));
    let _e623: f32 = wSurf;
    let _e625: f32 = hi_4;
    let _e626: f32 = lo_5;
    let _e633: f32 = k_7;
    let _e634: f32 = k_7;
    let _e639: f32 = k_7;
    depth_2 = ((max((_e623 - (0.5f * (_e625 + _e626))), 0f) * sqrt((1f + (_e633 * _e634)))) / _e639);
    let _e642: VoxelScene = u;
    let _e645: VoxelScene = u;
    let _e649: VoxelScene = u;
    let _e653: f32 = depth_2;
    cin = mix(_e642.waterSurfaceC.xyz, _e645.waterDeepC.xyz, vec3((1f - exp((-(_e649.waterL.x) * _e653)))));
    let _e660: vec3<f32> = wScatter;
    let _e661: vec3<f32> = (*trans_2);
    let _e663: vec3<f32> = T;
    let _e668: f32 = h_9;
    let _e671: vec3<f32> = cin;
    wScatter = (_e660 + (((_e661 * (vec3(1f) - _e663)) * (1f - _e668)) * _e671));
    let _e674: f32 = wHaze;
    let _e675: vec3<f32> = (*trans_2);
    let _e677: vec3<f32> = T;
    let _e681: f32 = h_9;
    wHaze = (_e674 + dot((_e675 * (vec3(1f) - _e677)), vec3((_e681 / 3f))));
    let _e687: vec3<f32> = (*trans_2);
    let _e688: vec3<f32> = T;
    (*trans_2) = (_e687 * _e688);
    return;
}

fn waterAt(x_84: i32, y_74: i32, z_86: i32, v_60: vec4<u32>, row_12: i32, acc_1: ptr<function, vec3<f32>>, trans_3: ptr<function, vec3<f32>>) {
    var x_85: i32;
    var y_75: i32;
    var z_87: i32;
    var v_61: vec4<u32>;
    var row_13: i32;
    var fill_2: i32;
    var R_3: i32;
    var bottom_2: i32;
    var skinRow_1: i32;
    var alpha_1: f32;
    var skinAlpha_1: f32;
    var openUp_3: bool;
    var dy_29: i32;
    var c_45: vec3<f32>;
    var a_13: f32;
    var nearSolid_1: bool;
    var local_81: i32;
    var nearFill_1: i32;
    var nearSkin_1: i32;
    var stop_1: i32;
    var nearOpenUp_1: bool;
    var local_82: i32;
    var isSkin_1: bool;
    var haze_4: f32;
    var local_83: vec3<f32>;
    var c_46: vec3<f32>;
    var local_84: f32;
    var a_14: f32;

    x_85 = x_84;
    y_75 = y_74;
    z_87 = z_86;
    v_61 = v_60;
    row_13 = row_12;
    if LIT {
        {
            let _e211: i32 = x_85;
            let _e212: i32 = y_75;
            let _e213: i32 = z_87;
            let _e214: vec4<u32> = v_61;
            let _e215: i32 = row_13;
            waterAtLit(_e211, _e212, _e213, _e214, _e215, acc_1, trans_3);
            return;
        }
    }
    let _e220: vec4<u32> = v_61;
    let _e223: i32 = fillPxQ(i32(_e220.y));
    fill_2 = _e223;
    let _e225: i32 = y_75;
    let _e226: i32 = z_87;
    let _e227: i32 = frontRow(_e225, _e226);
    R_3 = _e227;
    let _e229: i32 = R_3;
    let _e230: i32 = S;
    bottom_2 = (_e229 + _e230);
    let _e233: i32 = bottom_2;
    let _e234: i32 = fill_2;
    skinRow_1 = (_e233 - _e234);
    let _e237: VoxelScene = u;
    alpha_1 = clamp(_e237.knobs.y, 0f, 1f);
    let _e244: f32 = alpha_1;
    let _e245: VoxelScene = u;
    skinAlpha_1 = min((_e244 * _e245.waterK.x), 0.95f);
    let _e252: i32 = x_85;
    let _e253: i32 = y_75;
    let _e254: i32 = z_87;
    let _e255: bool = waterOpenUp(_e252, _e253, _e254);
    openUp_3 = _e255;
    let _e257: bool = openUp_3;
    let _e258: i32 = row_13;
    let _e259: i32 = skinRow_1;
    let _e260: i32 = RISE;
    let _e264: i32 = row_13;
    let _e265: i32 = skinRow_1;
    let _e268: i32 = x_85;
    let _e269: i32 = y_75;
    let _e270: i32 = z_87;
    let _e271: i32 = row_13;
    let _e272: bool = nearerOwns(_e268, _e269, _e270, _e271);
    if (((_e257 && (_e258 >= (_e259 - _e260))) && (_e264 < _e265)) && !(_e272)) {
        {
            let _e275: i32 = row_13;
            let _e276: i32 = skinRow_1;
            let _e277: i32 = RISE;
            dy_29 = (_e275 - (_e276 - _e277));
            let _e281: VoxelScene = u;
            let _e284: i32 = z_87;
            let _e286: i32 = RISE;
            let _e289: i32 = dy_29;
            let _e292: i32 = RISE;
            let _e296: f32 = hazeAt((f32(_e284) + (f32(((_e286 - 1i) - _e289)) / f32(_e292))));
            let _e297: vec3<f32> = hazed(_e281.waterSurfaceC.xyz, _e296);
            c_45 = _e297;
            let _e299: f32 = skinAlpha_1;
            let _e300: VoxelScene = u;
            a_13 = (_e299 * _e300.waterK.y);
            let _e305: vec3<f32> = (*acc_1);
            let _e306: vec3<f32> = (*trans_3);
            let _e307: f32 = a_13;
            let _e309: vec3<f32> = c_45;
            (*acc_1) = (_e305 + ((_e306 * _e307) * _e309));
            let _e312: vec3<f32> = (*trans_3);
            let _e314: f32 = a_13;
            (*trans_3) = (_e312 * (1f - _e314));
        }
    }
    let _e317: i32 = z_87;
    let _e320: i32 = x_85;
    let _e321: i32 = y_75;
    let _e322: i32 = z_87;
    let _e325: bool = solidAt(_e320, _e321, (_e322 - 1i));
    nearSolid_1 = ((_e317 > 0i) && _e325);
    let _e328: i32 = z_87;
    if (_e328 > 0i) {
        let _e331: i32 = x_85;
        let _e332: i32 = y_75;
        let _e333: i32 = z_87;
        let _e336: i32 = fillPxAt(_e331, _e332, (_e333 - 1i));
        local_81 = _e336;
    } else {
        local_81 = 0i;
    }
    let _e339: i32 = local_81;
    nearFill_1 = _e339;
    let _e341: i32 = R_3;
    let _e342: i32 = RISE;
    let _e344: i32 = S;
    let _e346: i32 = nearFill_1;
    nearSkin_1 = (((_e341 + _e342) + _e344) - _e346);
    let _e349: i32 = bottom_2;
    stop_1 = _e349;
    let _e351: bool = nearSolid_1;
    if _e351 {
        {
            let _e352: i32 = skinRow_1;
            stop_1 = _e352;
        }
    } else {
        let _e353: i32 = nearFill_1;
        if (_e353 > 0i) {
            {
                let _e356: i32 = x_85;
                let _e357: i32 = y_75;
                let _e358: i32 = z_87;
                let _e361: bool = waterOpenUp(_e356, _e357, (_e358 - 1i));
                nearOpenUp_1 = _e361;
                let _e363: i32 = bottom_2;
                let _e364: bool = nearOpenUp_1;
                if _e364 {
                    let _e365: i32 = nearSkin_1;
                    let _e366: i32 = RISE;
                    local_82 = (_e365 - _e366);
                } else {
                    let _e368: i32 = nearSkin_1;
                    local_82 = _e368;
                }
                let _e370: i32 = local_82;
                stop_1 = min(_e363, _e370);
            }
        }
    }
    let _e372: i32 = row_13;
    let _e373: i32 = skinRow_1;
    let _e375: i32 = row_13;
    let _e376: i32 = stop_1;
    if ((_e372 >= _e373) && (_e375 < _e376)) {
        {
            let _e379: i32 = row_13;
            let _e380: i32 = skinRow_1;
            let _e382: bool = openUp_3;
            isSkin_1 = ((_e379 == _e380) && _e382);
            let _e385: i32 = z_87;
            let _e387: f32 = hazeAt(f32(_e385));
            haze_4 = _e387;
            let _e389: bool = isSkin_1;
            if _e389 {
                let _e390: VoxelScene = u;
                local_83 = _e390.waterSurfaceC.xyz;
            } else {
                let _e393: VoxelScene = u;
                local_83 = _e393.waterDeepC.xyz;
            }
            let _e397: vec3<f32> = local_83;
            let _e398: f32 = haze_4;
            let _e399: vec3<f32> = hazed(_e397, _e398);
            c_46 = _e399;
            let _e401: bool = isSkin_1;
            if _e401 {
                let _e402: f32 = skinAlpha_1;
                local_84 = _e402;
            } else {
                let _e403: f32 = alpha_1;
                local_84 = _e403;
            }
            let _e405: f32 = local_84;
            a_14 = _e405;
            let _e407: vec3<f32> = (*acc_1);
            let _e408: vec3<f32> = (*trans_3);
            let _e409: f32 = a_14;
            let _e411: vec3<f32> = c_46;
            (*acc_1) = (_e407 + ((_e408 * _e409) * _e411));
            let _e414: vec3<f32> = (*trans_3);
            let _e416: f32 = a_14;
            (*trans_3) = (_e414 * (1f - _e416));
            return;
        }
    } else {
        return;
    }
}

fn bandAt(t_23: f32) -> vec3<f32> {
    var t_24: f32;
    var b_8: f32;
    var c_47: vec3<f32>;

    t_24 = t_23;
    let _e203: f32 = t_24;
    b_8 = smoothstep(0.04f, 0.56f, _e203);
    let _e206: VoxelScene = u;
    let _e209: VoxelScene = u;
    let _e214: f32 = b_8;
    c_47 = mix(_e206.bandA.xyz, _e209.bandB.xyz, vec3(smoothstep(0f, 0.5f, _e214)));
    let _e219: vec3<f32> = c_47;
    let _e220: VoxelScene = u;
    let _e225: f32 = b_8;
    c_47 = mix(_e219, _e220.bandC.xyz, vec3(smoothstep(0.5f, 1f, _e225)));
    let _e229: vec3<f32> = c_47;
    let _e230: f32 = b_8;
    let _e231: f32 = b_8;
    let _e233: VoxelScene = u;
    return (_e229 * ((_e230 * _e231) * _e233.dayK.w));
}

fn skyAt(py_2: i32) -> vec3<f32> {
    var py_3: i32;
    var t_25: f32;
    var c_48: vec3<f32>;

    py_3 = py_2;
    let _e201: VoxelScene = u;
    if (_e201.roofK.w > 0.5f) {
        {
            let _e206: i32 = py_3;
            let _e207: VoxelScene = u;
            let _e212: VoxelScene = u;
            t_25 = clamp((f32((_e206 + _e207.camera.x)) / f32(max(_e212.camera.y, 1i))), 0f, 1f);
            let _e223: VoxelScene = u;
            let _e226: VoxelScene = u;
            let _e229: f32 = t_25;
            c_48 = mix(_e223.skyC.xyz, _e226.skyHorizon.xyz, vec3(_e229));
            let _e233: VoxelScene = u;
            if (_e233.dayK.w > 0f) {
                {
                    let _e238: vec3<f32> = c_48;
                    let _e239: f32 = t_25;
                    let _e240: vec3<f32> = bandAt(_e239);
                    c_48 = (_e238 + _e240);
                }
            }
            let _e242: vec3<f32> = c_48;
            return _e242;
        }
    }
    let _e243: VoxelScene = u;
    return _e243.skyC.xyz;
}

fn skyColor(px_x: i32, px_y: i32) -> vec3<f32> {
    var px_x_1: i32;
    var px_y_1: i32;
    var sky: vec3<f32>;
    var moisture: f32;
    var mistRows: i32;
    var verticalT: f32;
    var rowFactor: f32;
    var driftX: i32;
    var wave1_: f32;
    var wave2_: f32;
    var density: f32;
    var a_15: f32;
    var cloudC: vec3<f32> = vec3<f32>(0.038f, 0.02f, 0.102f);
    var cloudEdge: vec3<f32> = vec3<f32>(0.113f, 0.171f, 0.354f);
    var col_1: vec3<f32>;

    px_x_1 = px_x;
    px_y_1 = px_y;
    if LIT {
        {
            let _e203: i32 = px_x_1;
            let _e204: i32 = px_y_1;
            let _e207: vec4<f32> = textureLoad(wgpu_backdropTex, vec2<i32>(_e203, _e204), 0i);
            return _e207.xyz;
        }
    }
    let _e209: i32 = px_y_1;
    let _e210: vec3<f32> = skyAt(_e209);
    sky = _e210;
    let _e212: VoxelScene = u;
    moisture = clamp(_e212.knobs.z, 0f, 1f);
    let _e219: VoxelScene = u;
    mistRows = min((_e219.extent.w / 3i), 56i);
    let _e227: VoxelScene = u;
    let _e232: f32 = moisture;
    let _e236: i32 = px_y_1;
    let _e237: i32 = mistRows;
    if (((_e227.roofK.w > 0.5f) && (_e232 > 0f)) && (_e236 < _e237)) {
        {
            let _e241: i32 = px_y_1;
            let _e243: i32 = mistRows;
            verticalT = (1f - (f32(_e241) / f32(_e243)));
            let _e248: f32 = verticalT;
            let _e249: f32 = verticalT;
            let _e251: f32 = moisture;
            rowFactor = ((_e248 * _e249) * _e251);
            let _e254: VoxelScene = u;
            driftX = (i32(_e254.knobs.w) / 2i);
            let _e261: i32 = px_x_1;
            let _e262: i32 = driftX;
            let _e264: i32 = px_y_1;
            wave1_ = ((sin((f32(((_e261 + _e262) + (_e264 * 4i))) * 0.045f)) * 0.5f) + 0.5f);
            let _e277: i32 = px_x_1;
            let _e280: i32 = driftX;
            wave2_ = ((cos((f32((((_e277 * 2i) - _e280) + 37i)) * 0.025f)) * 0.5f) + 0.5f);
            let _e293: f32 = wave1_;
            let _e294: f32 = wave2_;
            let _e296: f32 = rowFactor;
            density = ((_e293 * _e294) * _e296);
            let _e299: f32 = density;
            if (_e299 > 0.1f) {
                {
                    let _e302: f32 = density;
                    a_15 = min(((_e302 - 0.1f) * 1.8f), 0.7f);
                    let _e320: vec3<f32> = cloudC;
                    let _e321: vec3<f32> = cloudEdge;
                    let _e322: f32 = wave1_;
                    col_1 = mix(_e320, _e321, vec3((_e322 * 0.5f)));
                    let _e328: vec3<f32> = sky;
                    let _e329: vec3<f32> = col_1;
                    let _e330: f32 = a_15;
                    sky = mix(_e328, _e329, vec3(_e330));
                }
            }
        }
    }
    let _e333: vec3<f32> = sky;
    return _e333;
}

fn surfaceNear(x_86: i32, y_76: i32, z_88: i32) -> f32 {
    var x_87: i32;
    var y_77: i32;
    var z_89: i32;
    var local_85: i32;
    var ga: i32;
    var g_1: i32;
    var yy_1: i32;
    var gg: i32;

    x_87 = x_86;
    y_77 = y_76;
    z_89 = z_88;
    let _e208: i32 = z_89;
    let _e211: i32 = z_89;
    let _e212: i32 = D;
    let _e215: i32 = x_87;
    let _e216: i32 = y_77;
    let _e217: i32 = z_89;
    let _e218: bool = solidAt(_e215, _e216, _e217);
    if (((_e208 < 0i) || (_e211 >= _e212)) || _e218) {
        {
            return WALL;
        }
    }
    let _e220: i32 = x_87;
    let _e221: i32 = y_77;
    let _e224: i32 = z_89;
    let _e225: bool = solidAt(_e220, (_e221 + 1i), _e224);
    if _e225 {
        local_85 = 0i;
    } else {
        let _e227: i32 = x_87;
        let _e228: i32 = y_77;
        let _e231: i32 = z_89;
        let _e232: i32 = freeQ(_e227, (_e228 + 1i), _e231);
        local_85 = _e232;
    }
    let _e234: i32 = local_85;
    ga = _e234;
    let _e236: i32 = ga;
    if (_e236 > 0i) {
        {
            let _e239: i32 = y_77;
            let _e243: i32 = ga;
            return (f32((_e239 + 1i)) + (f32(_e243) / 255f));
        }
    }
    let _e248: i32 = x_87;
    let _e249: i32 = y_77;
    let _e250: i32 = z_89;
    let _e251: i32 = freeQ(_e248, _e249, _e250);
    g_1 = _e251;
    let _e253: i32 = g_1;
    if (_e253 > 0i) {
        {
            let _e256: i32 = y_77;
            let _e258: i32 = g_1;
            return (f32(_e256) + (f32(_e258) / 255f));
        }
    }
    let _e263: i32 = y_77;
    yy_1 = (_e263 - 1i);
    loop {
        let _e267: i32 = yy_1;
        let _e268: i32 = y_77;
        if !((_e267 >= max((_e268 - 2i), 0i))) {
            break;
        }
        {
            let _e278: i32 = x_87;
            let _e279: i32 = yy_1;
            let _e280: i32 = z_89;
            let _e281: bool = solidAt(_e278, _e279, _e280);
            if _e281 {
                {
                    let _e282: i32 = yy_1;
                    return f32((_e282 + 1i));
                }
            }
            let _e286: i32 = x_87;
            let _e287: i32 = yy_1;
            let _e288: i32 = z_89;
            let _e289: i32 = freeQ(_e286, _e287, _e288);
            gg = _e289;
            let _e291: i32 = gg;
            if (_e291 > 0i) {
                {
                    let _e294: i32 = yy_1;
                    let _e296: i32 = gg;
                    return (f32(_e294) + (f32(_e296) / 255f));
                }
            }
        }
        continuing {
            let _e275: i32 = yy_1;
            yy_1 = (_e275 - 1i);
        }
    }
    let _e301: i32 = y_77;
    return f32(max((_e301 - 2i), 0i));
}

fn slope1_(a_16: f32, h0_: f32, b_9: f32) -> f32 {
    var a_17: f32;
    var h0_1: f32;
    var b_10: f32;
    var wa: bool;
    var wb: bool;

    a_17 = a_16;
    h0_1 = h0_;
    b_10 = b_9;
    let _e208: f32 = a_17;
    wa = (_e208 < -500000f);
    let _e214: f32 = b_10;
    wb = (_e214 < -500000f);
    let _e220: bool = wa;
    let _e221: bool = wb;
    if (_e220 && _e221) {
        {
            return 0f;
        }
    }
    let _e224: bool = wa;
    if _e224 {
        {
            let _e225: f32 = b_10;
            let _e226: f32 = h0_1;
            return (_e225 - _e226);
        }
    }
    let _e228: bool = wb;
    if _e228 {
        {
            let _e229: f32 = h0_1;
            let _e230: f32 = a_17;
            return (_e229 - _e230);
        }
    }
    let _e233: f32 = b_10;
    let _e234: f32 = a_17;
    return (0.5f * (_e233 - _e234));
}

fn flowAt(c_49: vec3<i32>) -> vec3<f32> {
    var c_50: vec3<i32>;
    var x_88: i32;
    var y_78: i32;
    var z_90: i32;
    var h0_2: f32;
    var gx: f32;
    var gz: f32;
    var f_2: vec2<f32>;
    var m_13: f32;
    var q_10: f32;
    var local_86: vec2<f32>;

    c_50 = c_49;
    let _e204: vec3<i32> = c_50;
    x_88 = _e204.x;
    let _e207: vec3<i32> = c_50;
    y_78 = _e207.y;
    let _e210: vec3<i32> = c_50;
    z_90 = _e210.z;
    let _e213: i32 = y_78;
    let _e216: i32 = x_88;
    let _e217: i32 = y_78;
    let _e220: i32 = z_90;
    let _e221: bool = solidAt(_e216, (_e217 - 1i), _e220);
    let _e224: i32 = x_88;
    let _e225: i32 = y_78;
    let _e228: i32 = z_90;
    let _e229: i32 = fillPxAt(_e224, (_e225 - 1i), _e228);
    let _e230: i32 = S;
    if (((_e213 > 0i) && !(_e221)) && (_e229 < _e230)) {
        {
            return vec3<f32>(0f, 0f, 1f);
        }
    }
    let _e237: i32 = x_88;
    let _e238: i32 = y_78;
    let _e239: i32 = z_90;
    let _e240: f32 = surfaceNear(_e237, _e238, _e239);
    h0_2 = _e240;
    let _e242: i32 = x_88;
    let _e245: i32 = y_78;
    let _e246: i32 = z_90;
    let _e247: f32 = surfaceNear((_e242 - 1i), _e245, _e246);
    let _e248: f32 = h0_2;
    let _e249: i32 = x_88;
    let _e252: i32 = y_78;
    let _e253: i32 = z_90;
    let _e254: f32 = surfaceNear((_e249 + 1i), _e252, _e253);
    let _e255: f32 = slope1_(_e247, _e248, _e254);
    gx = _e255;
    let _e257: i32 = x_88;
    let _e258: i32 = y_78;
    let _e259: i32 = z_90;
    let _e262: f32 = surfaceNear(_e257, _e258, (_e259 - 1i));
    let _e263: f32 = h0_2;
    let _e264: i32 = x_88;
    let _e265: i32 = y_78;
    let _e266: i32 = z_90;
    let _e269: f32 = surfaceNear(_e264, _e265, (_e266 + 1i));
    let _e270: f32 = slope1_(_e262, _e263, _e269);
    gz = _e270;
    let _e272: f32 = gx;
    let _e273: f32 = gz;
    f_2 = -(vec2<f32>(_e272, _e273));
    let _e277: vec2<f32> = f_2;
    m_13 = length(_e277);
    let _e280: f32 = m_13;
    q_10 = (floor(((clamp((_e280 / FLOW_FULL), 0f, 1f) * FLOW_STEPS) + 0.5f)) / FLOW_STEPS);
    let _e291: f32 = m_13;
    if (_e291 > 0f) {
        let _e294: vec2<f32> = f_2;
        let _e295: f32 = m_13;
        let _e298: f32 = q_10;
        local_86 = ((_e294 / vec2(_e295)) * _e298);
    } else {
        local_86 = vec2(0f);
    }
    let _e303: vec2<f32> = local_86;
    return vec3<f32>(_e303.x, _e303.y, 0f);
}

fn wrapI(a_18: i32, m_14: i32) -> i32 {
    var a_19: i32;
    var m_15: i32;

    a_19 = a_18;
    m_15 = m_14;
    let _e206: i32 = a_19;
    let _e207: i32 = m_15;
    let _e208: i32 = wrapMod(_e206, _e207);
    return _e208;
}

fn rippleLine(w_5: vec2<f32>, flow: vec2<f32>, phase: f32, cycle: i32, salt_8: i32) -> vec2<f32> {
    var w_6: vec2<f32>;
    var flow_1: vec2<f32>;
    var phase_1: f32;
    var cycle_1: i32;
    var salt_9: i32;
    var t_26: f32;
    var a_20: vec2<f32>;
    var row_14: i32;
    var hr: u32;
    var segs: i32;
    var sx: f32;
    var seg: i32;
    var h_10: u32;
    var hl: u32;
    var life: f32;
    var age: f32;
    var fade: f32;
    var f_3: f32;
    var mid: f32;
    var halfLen: f32;
    var along: f32;
    var taper: f32;
    var local_87: f32;

    w_6 = w_5;
    flow_1 = flow;
    phase_1 = phase;
    cycle_1 = cycle;
    salt_9 = salt_8;
    let _e220: f32 = phase_1;
    t_26 = ((_e220 * FLOW_CYCLE) * FLOW_ADVECT);
    let _e224: vec2<f32> = w_6;
    let _e225: vec2<f32> = flow_1;
    let _e226: f32 = t_26;
    a_20 = (_e224 - (_e225 * _e226));
    let _e230: vec2<f32> = a_20;
    let _e232: i32 = RISE;
    row_14 = i32(floor((_e230.y * f32(_e232))));
    let _e238: i32 = row_14;
    let _e240: i32 = cycle_1;
    let _e241: i32 = salt_9;
    let _e242: u32 = cellHash(_e238, 0i, _e240, _e241);
    hr = _e242;
    let _e245: vec2<f32> = a_20;
    let _e247: u32 = hr;
    let _e257: f32 = t_26;
    a_20.x = (_e245.x - (mix(BREEZE_MIN, BREEZE_MAX, (f32(((_e247 >> 8u) & 255u)) / 255f)) * _e257));
    let _e261: i32 = W;
    segs = max(1i, i32(((f32(_e261) / LINE_SEG) + 0.5f)));
    let _e269: vec2<f32> = a_20;
    let _e271: i32 = segs;
    let _e274: i32 = W;
    let _e277: u32 = hr;
    sx = (((_e269.x * f32(_e271)) / f32(_e274)) + (f32((_e277 & 255u)) / 256f));
    let _e285: f32 = sx;
    seg = i32(floor(_e285));
    let _e289: i32 = seg;
    let _e290: i32 = segs;
    let _e291: i32 = wrapI(_e289, _e290);
    let _e292: i32 = row_14;
    let _e293: i32 = cycle_1;
    let _e294: i32 = salt_9;
    let _e297: u32 = cellHash(_e291, _e292, _e293, (_e294 + 1i));
    h_10 = _e297;
    let _e299: u32 = h_10;
    if ((f32((_e299 & 65535u)) / 65535f) >= LINE_DENSITY) {
        {
            return vec2(0f);
        }
    }
    let _e308: i32 = seg;
    let _e309: i32 = segs;
    let _e310: i32 = wrapI(_e308, _e309);
    let _e311: i32 = row_14;
    let _e312: i32 = cycle_1;
    let _e313: i32 = salt_9;
    let _e316: u32 = cellHash(_e310, _e311, _e312, (_e313 + 2i));
    hl = _e316;
    let _e318: u32 = hl;
    life = mix(LINE_LIFE_MIN, LINE_LIFE_MAX, (f32((_e318 & 255u)) / 255f));
    let _e326: f32 = phase_1;
    let _e327: u32 = hl;
    let _e337: f32 = life;
    let _e341: f32 = life;
    age = ((_e326 - ((f32(((_e327 >> 8u) & 255u)) / 255f) * (1f - _e337))) / _e341);
    let _e344: f32 = age;
    let _e347: f32 = age;
    if ((_e344 <= 0f) || (_e347 >= 1f)) {
        {
            return vec2(0f);
        }
    }
    let _e354: f32 = age;
    fade = sin((3.1415927f * _e354));
    let _e358: f32 = fade;
    let _e359: f32 = fade;
    fade = (_e358 * _e359);
    let _e361: f32 = sx;
    let _e362: i32 = seg;
    f_3 = (_e361 - f32(_e362));
    let _e367: u32 = h_10;
    mid = (0.25f + ((f32(((_e367 >> 16u) & 255u)) / 255f) * 0.5f));
    let _e381: u32 = h_10;
    let _e393: f32 = fade;
    halfLen = ((0.1f + ((f32(((_e381 >> 24u) & 127u)) / 127f) * 0.15f)) * sqrt(_e393));
    let _e397: f32 = f_3;
    let _e398: f32 = mid;
    let _e401: f32 = halfLen;
    if (abs((_e397 - _e398)) >= _e401) {
        {
            return vec2(0f);
        }
    }
    let _e405: f32 = f_3;
    let _e406: f32 = mid;
    let _e408: f32 = halfLen;
    along = ((_e405 - _e406) / _e408);
    let _e412: f32 = along;
    let _e413: f32 = along;
    taper = (1f - (_e412 * _e413));
    let _e417: u32 = h_10;
    if ((_e417 & 2147483648u) != 0u) {
        local_87 = 1f;
    } else {
        local_87 = -1f;
    }
    let _e426: f32 = local_87;
    let _e427: f32 = fade;
    let _e429: f32 = fade;
    let _e430: f32 = taper;
    let _e432: f32 = taper;
    return vec2<f32>((_e426 * _e427), ((_e429 * _e430) * _e432));
}

fn rippleNormal(w_7: vec2<f32>, flow_2: vec2<f32>, glint: ptr<function, f32>) -> vec3<f32> {
    var w_8: vec2<f32>;
    var flow_3: vec2<f32>;
    var st: f32;
    var c1_: f32;
    var c2_: f32;
    var s_2: vec2<f32>;
    var n_4: vec3<f32>;

    w_8 = w_7;
    flow_3 = flow_2;
    let _e215: VoxelScene = u;
    st = _e215.clock.y;
    let _e219: f32 = st;
    c1_ = (_e219 / FLOW_CYCLE);
    let _e222: f32 = c1_;
    c2_ = (_e222 + 0.5f);
    let _e226: vec2<f32> = w_8;
    let _e227: vec2<f32> = flow_3;
    let _e228: f32 = c1_;
    let _e230: f32 = c1_;
    let _e234: i32 = wrapI(i32(floor(_e230)), 32i);
    let _e236: vec2<f32> = rippleLine(_e226, _e227, fract(_e228), _e234, 60i);
    s_2 = _e236;
    let _e238: vec2<f32> = s_2;
    if (_e238.x == 0f) {
        {
            let _e242: vec2<f32> = w_8;
            let _e247: vec2<f32> = flow_3;
            let _e248: f32 = c2_;
            let _e250: f32 = c2_;
            let _e254: i32 = wrapI(i32(floor(_e250)), 32i);
            let _e256: vec2<f32> = rippleLine((_e242 + vec2<f32>(0.37f, 0f)), _e247, fract(_e248), _e254, 62i);
            s_2 = _e256;
        }
    }
    (*glint) = 0f;
    let _e258: vec2<f32> = s_2;
    if (_e258.x == 0f) {
        {
            return vec3<f32>(0f, 1f, 0f);
        }
    }
    let _e268: vec2<f32> = s_2;
    let _e271: VoxelScene = u;
    n_4 = normalize(vec3<f32>(0f, 1f, (-(_e268.x) * _e271.waterL.z)));
    let _e278: vec3<f32> = n_4;
    let _e280: VoxelScene = u;
    if ((_e278.z * _e280.sunK.z) > 0f) {
        {
            let _e286: vec2<f32> = s_2;
            (*glint) = _e286.y;
        }
    }
    let _e288: vec3<f32> = n_4;
    return _e288;
}

fn hash01_(x_89: i32, y_79: i32, z_91: i32, salt_10: i32) -> f32 {
    var x_90: i32;
    var y_80: i32;
    var z_92: i32;
    var salt_11: i32;

    x_90 = x_89;
    y_80 = y_79;
    z_92 = z_91;
    salt_11 = salt_10;
    let _e218: i32 = x_90;
    let _e219: i32 = y_80;
    let _e220: i32 = z_92;
    let _e221: i32 = salt_11;
    let _e222: u32 = cellHash(_e218, _e219, _e220, _e221);
    return (f32((_e222 & 65535u)) / 65535f);
}

fn vnoise(w_9: vec2<f32>, freq: f32, layer: i32, salt_12: i32) -> f32 {
    var w_10: vec2<f32>;
    var freq_1: f32;
    var layer_1: i32;
    var salt_13: i32;
    var period: i32;
    var q_11: vec2<f32>;
    var i_10: vec2<f32>;
    var f_4: vec2<f32>;
    var x0_: i32;
    var x1_: i32;
    var y0_: i32;
    var a_21: f32;
    var b_11: f32;
    var c_51: f32;
    var d: f32;

    w_10 = w_9;
    freq_1 = freq;
    layer_1 = layer;
    salt_13 = salt_12;
    let _e219: i32 = W;
    let _e221: f32 = freq_1;
    period = max(1i, i32(((f32(_e219) * _e221) + 0.5f)));
    let _e228: vec2<f32> = w_10;
    let _e230: i32 = period;
    let _e233: i32 = W;
    let _e236: vec2<f32> = w_10;
    let _e238: f32 = freq_1;
    q_11 = vec2<f32>(((_e228.x * f32(_e230)) / f32(_e233)), (_e236.y * _e238));
    let _e242: vec2<f32> = q_11;
    i_10 = floor(_e242);
    let _e245: vec2<f32> = q_11;
    let _e246: vec2<f32> = i_10;
    f_4 = (_e245 - _e246);
    let _e249: vec2<f32> = f_4;
    let _e250: vec2<f32> = f_4;
    let _e254: vec2<f32> = f_4;
    f_4 = ((_e249 * _e250) * (vec2(3f) - (2f * _e254)));
    let _e259: vec2<f32> = i_10;
    let _e262: i32 = period;
    let _e263: i32 = wrapI(i32(_e259.x), _e262);
    x0_ = _e263;
    let _e265: vec2<f32> = i_10;
    let _e270: i32 = period;
    let _e271: i32 = wrapI((i32(_e265.x) + 1i), _e270);
    x1_ = _e271;
    let _e273: vec2<f32> = i_10;
    y0_ = i32(_e273.y);
    let _e277: i32 = x0_;
    let _e278: i32 = y0_;
    let _e279: i32 = layer_1;
    let _e280: i32 = salt_13;
    let _e281: f32 = hash01_(_e277, _e278, _e279, _e280);
    a_21 = _e281;
    let _e283: i32 = x1_;
    let _e284: i32 = y0_;
    let _e285: i32 = layer_1;
    let _e286: i32 = salt_13;
    let _e287: f32 = hash01_(_e283, _e284, _e285, _e286);
    b_11 = _e287;
    let _e289: i32 = x0_;
    let _e290: i32 = y0_;
    let _e293: i32 = layer_1;
    let _e294: i32 = salt_13;
    let _e295: f32 = hash01_(_e289, (_e290 + 1i), _e293, _e294);
    c_51 = _e295;
    let _e297: i32 = x1_;
    let _e298: i32 = y0_;
    let _e301: i32 = layer_1;
    let _e302: i32 = salt_13;
    let _e303: f32 = hash01_(_e297, (_e298 + 1i), _e301, _e302);
    d = _e303;
    let _e305: f32 = a_21;
    let _e306: f32 = b_11;
    let _e307: vec2<f32> = f_4;
    let _e310: f32 = c_51;
    let _e311: f32 = d;
    let _e312: vec2<f32> = f_4;
    let _e315: vec2<f32> = f_4;
    return mix(mix(_e305, _e306, _e307.x), mix(_e310, _e311, _e312.x), _e315.y);
}

fn vnoiseT(w_11: vec2<f32>, freq_2: f32, t_27: f32, salt_14: i32) -> f32 {
    var w_12: vec2<f32>;
    var freq_3: f32;
    var t_28: f32;
    var salt_15: i32;
    var lt: f32;
    var ft: f32;
    var l0_: i32;
    var l1_: i32;

    w_12 = w_11;
    freq_3 = freq_2;
    t_28 = t_27;
    salt_15 = salt_14;
    let _e218: f32 = t_28;
    lt = floor(_e218);
    let _e221: f32 = t_28;
    let _e222: f32 = lt;
    ft = (_e221 - _e222);
    let _e225: f32 = ft;
    let _e226: f32 = ft;
    let _e230: f32 = ft;
    ft = ((_e225 * _e226) * (3f - (2f * _e230)));
    let _e234: f32 = lt;
    let _e237: i32 = wrapI(i32(_e234), 256i);
    l0_ = _e237;
    let _e239: f32 = lt;
    let _e244: i32 = wrapI((i32(_e239) + 1i), 256i);
    l1_ = _e244;
    let _e246: vec2<f32> = w_12;
    let _e247: f32 = freq_3;
    let _e248: i32 = l0_;
    let _e249: i32 = salt_15;
    let _e250: f32 = vnoise(_e246, _e247, _e248, _e249);
    let _e251: vec2<f32> = w_12;
    let _e252: f32 = freq_3;
    let _e253: i32 = l1_;
    let _e254: i32 = salt_15;
    let _e255: f32 = vnoise(_e251, _e252, _e253, _e254);
    let _e256: f32 = ft;
    return mix(_e250, _e255, _e256);
}

fn starAt(px: i32, py_4: i32, t_29: f32) -> vec4<f32> {
    var px_1: i32;
    var py_5: i32;
    var t_30: f32;
    var C: i32;
    var cell: vec2<i32>;
    var h_11: u32;
    var g_2: u32;
    var f_5: u32;
    var r_20: f32;
    var b_12: f32;
    var local_88: i32;
    var sz: i32;
    var span_1: i32;
    var at_1: vec2<i32>;
    var d_1: vec2<i32>;
    var k_8: f32;
    var w1_: f32;
    var w2_: f32;
    var ph1_: f32;
    var ph2_: f32;
    var wave: f32;
    var faint: f32;
    var depth_3: f32;
    var tw: f32;
    var temp: f32;
    var local_89: vec3<f32>;
    var c_52: vec3<f32>;
    var rare: u32;
    var lum: f32;

    px_1 = px;
    py_5 = py_4;
    t_30 = t_29;
    let _e224: i32 = S;
    C = max(4i, i32(((f32(_e224) * STAR_CELL) + 0.5f)));
    let _e232: i32 = px_1;
    let _e233: i32 = C;
    let _e235: i32 = py_5;
    let _e236: i32 = C;
    cell = vec2<i32>((_e232 / _e233), (_e235 / _e236));
    let _e240: vec2<i32> = cell;
    let _e242: vec2<i32> = cell;
    let _e246: u32 = cellHash(_e240.x, _e242.y, 0i, 71i);
    h_11 = _e246;
    let _e248: u32 = h_11;
    if ((f32((_e248 & 65535u)) / 65536f) >= STAR_SHARE) {
        {
            return vec4(0f);
        }
    }
    let _e257: vec2<i32> = cell;
    let _e259: vec2<i32> = cell;
    let _e263: u32 = cellHash(_e257.x, _e259.y, 1i, 72i);
    g_2 = _e263;
    let _e265: vec2<i32> = cell;
    let _e267: vec2<i32> = cell;
    let _e271: u32 = cellHash(_e265.x, _e267.y, 2i, 73i);
    f_5 = _e271;
    let _e273: u32 = f_5;
    r_20 = ((f32((_e273 & 65535u)) + 1f) / 65536f);
    let _e282: f32 = r_20;
    b_12 = min((STAR_MIN * pow(_e282, -0.6666667f)), STAR_MAX);
    let _e291: i32 = S;
    if (_e291 >= 10i) {
        local_88 = 2i;
    } else {
        local_88 = 1i;
    }
    let _e297: i32 = local_88;
    sz = _e297;
    let _e299: i32 = C;
    let _e300: i32 = sz;
    span_1 = max(((_e299 - _e300) - 1i), 1i);
    let _e308: u32 = g_2;
    let _e309: i32 = span_1;
    let _e315: u32 = g_2;
    let _e319: i32 = span_1;
    at_1 = vec2<i32>((1i + i32((_e308 % u32(_e309)))), (1i + i32(((_e315 >> 8u) % u32(_e319)))));
    let _e326: i32 = px_1;
    let _e327: i32 = py_5;
    let _e329: vec2<i32> = cell;
    let _e330: i32 = C;
    let _e333: vec2<i32> = at_1;
    d_1 = ((vec2<i32>(_e326, _e327) - (_e329 * _e330)) - _e333);
    let _e337: vec2<i32> = d_1;
    let _e341: vec2<i32> = d_1;
    let _e346: vec2<i32> = d_1;
    let _e348: i32 = sz;
    let _e351: vec2<i32> = d_1;
    let _e353: i32 = sz;
    if ((((_e337.x >= 0i) && (_e341.y >= 0i)) && (_e346.x < _e348)) && (_e351.y < _e353)) {
        {
            k_8 = 1f;
        }
    } else {
        let _e357: f32 = b_12;
        let _e359: vec2<i32> = d_1;
        let _e364: vec2<i32> = d_1;
        let _e366: i32 = sz;
        let _e369: vec2<i32> = d_1;
        let _e374: vec2<i32> = d_1;
        let _e376: i32 = sz;
        let _e379: vec2<i32> = d_1;
        let _e384: vec2<i32> = d_1;
        let _e386: i32 = sz;
        let _e389: vec2<i32> = d_1;
        let _e394: vec2<i32> = d_1;
        let _e396: i32 = sz;
        if ((_e357 > STAR_BRIGHT) && (((((_e359.x == -1i) || (_e364.x == _e366)) && (_e369.y >= 0i)) && (_e374.y < _e376)) || ((((_e379.y == -1i) || (_e384.y == _e386)) && (_e389.x >= 0i)) && (_e394.x < _e396)))) {
            {
                let _e402: f32 = b_12;
                k_8 = (0.22f * min(((_e402 - STAR_BRIGHT) / STAR_BRIGHT), 1f));
            }
        } else {
            {
                return vec4(0f);
            }
        }
    }
    let _e412: u32 = g_2;
    w1_ = ((6.2831855f * f32((720u + ((_e412 >> 16u) % 2520u)))) / 3600f);
    let _e426: u32 = f_5;
    w2_ = ((6.2831855f * f32((250u + ((_e426 >> 16u) % 830u)))) / 3600f);
    let _e438: u32 = h_11;
    ph1_ = (f32((_e438 >> 16u)) * 0.0000958738f);
    let _e446: u32 = g_2;
    ph2_ = (f32((_e446 >> 24u)) * 0.024543693f);
    let _e457: VoxelScene = u;
    let _e460: f32 = w1_;
    let _e462: f32 = ph1_;
    let _e467: VoxelScene = u;
    let _e470: f32 = w2_;
    let _e472: f32 = ph2_;
    wave = (0.5f + (0.5f * ((0.65f * sin(((_e457.cloudK.w * _e460) + _e462))) + (0.35f * sin(((_e467.cloudK.w * _e470) + _e472))))));
    let _e481: f32 = b_12;
    faint = (1f - smoothstep(STAR_MIN, STAR_BRIGHT, _e481));
    let _e487: f32 = faint;
    let _e493: f32 = t_30;
    depth_3 = clamp((mix(0.18f, 0.55f, _e487) * (1f + (1.2f * smoothstep(0.1f, 0.5f, _e493)))), 0f, 0.9f);
    let _e503: f32 = depth_3;
    let _e504: f32 = wave;
    tw = (1f - (_e503 * _e504));
    let _e508: u32 = f_5;
    temp = (f32(((_e508 >> 16u) & 255u)) / 255f);
    let _e518: f32 = temp;
    if (_e518 < 0.55f) {
        let _e527: f32 = temp;
        local_89 = mix(vec3<f32>(0.72f, 0.84f, 1f), vec3(1f), vec3((_e527 / 0.55f)));
    } else {
        let _e538: f32 = temp;
        local_89 = mix(vec3(1f), vec3<f32>(1f, 0.82f, 0.62f), vec3(((_e538 - 0.55f) / 0.45f)));
    }
    let _e546: vec3<f32> = local_89;
    c_52 = _e546;
    let _e548: u32 = f_5;
    rare = ((_e548 >> 24u) % 16u);
    let _e555: u32 = rare;
    if (_e555 == 0u) {
        {
            let _e558: vec3<f32> = c_52;
            let _e559: VoxelScene = u;
            let _e562: VoxelScene = u;
            let _e565: VoxelScene = u;
            let _e569: VoxelScene = u;
            c_52 = mix(_e558, (_e559.lightC.xyz / vec3(max(max(_e562.lightC.x, _e565.lightC.y), max(_e569.lightC.z, 0.001f)))), vec3(0.5f));
        }
    }
    let _e580: u32 = rare;
    if (_e580 == 1u) {
        {
            let _e583: vec3<f32> = c_52;
            let _e584: VoxelScene = u;
            let _e587: VoxelScene = u;
            let _e590: VoxelScene = u;
            let _e594: VoxelScene = u;
            c_52 = mix(_e583, (_e584.bandB.xyz / vec3(max(max(_e587.bandB.x, _e590.bandB.y), max(_e594.bandB.z, 0.001f)))), vec3(0.4f));
        }
    }
    let _e605: f32 = b_12;
    let _e606: f32 = tw;
    let _e608: f32 = k_8;
    lum = ((_e605 * _e606) * _e608);
    let _e611: vec3<f32> = c_52;
    let _e612: f32 = lum;
    let _e613: vec3<f32> = (_e611 * _e612);
    let _e614: f32 = k_8;
    let _e615: f32 = b_12;
    return vec4<f32>(_e613.x, _e613.y, _e613.z, ((_e614 * max((_e615 - STAR_BRIGHT), 0f)) / 2.1f));
}

fn cloudFbm(q_12: vec2<f32>, octaves: i32, salt_16: i32) -> f32 {
    var q_13: vec2<f32>;
    var octaves_1: i32;
    var salt_17: i32;
    var sum_2: f32 = 0f;
    var amp_6: f32 = 0.5f;
    var norm: f32 = 0f;
    var freq_4: f32 = CLOUD_FREQ;
    var i_11: i32 = 0i;

    q_13 = q_12;
    octaves_1 = octaves;
    salt_17 = salt_16;
    loop {
        let _e235: i32 = i_11;
        let _e236: i32 = octaves_1;
        if !((_e235 < _e236)) {
            break;
        }
        {
            let _e242: f32 = sum_2;
            let _e243: f32 = amp_6;
            let _e244: vec2<f32> = q_13;
            let _e245: f32 = freq_4;
            let _e246: i32 = i_11;
            let _e247: i32 = salt_17;
            let _e248: f32 = vnoise(_e244, _e245, _e246, _e247);
            sum_2 = (_e242 + (_e243 * _e248));
            let _e251: f32 = norm;
            let _e252: f32 = amp_6;
            norm = (_e251 + _e252);
            let _e254: f32 = amp_6;
            amp_6 = (_e254 * 0.5f);
            let _e257: f32 = freq_4;
            freq_4 = (_e257 * 2f);
        }
        continuing {
            let _e239: i32 = i_11;
            i_11 = (_e239 + 1i);
        }
    }
    let _e260: f32 = sum_2;
    let _e261: f32 = norm;
    return (_e260 / _e261);
}

fn cloudNoise(q_14: vec2<f32>) -> f32 {
    var q_15: vec2<f32>;
    var wq: vec2<f32>;
    var warp: vec2<f32>;

    q_15 = q_14;
    let _e222: vec2<f32> = q_15;
    let _e224: VoxelScene = u;
    wq = (_e222 - vec2<f32>((0.5f * _e224.cloudK.y), 0f));
    let _e232: vec2<f32> = wq;
    let _e235: f32 = cloudFbm(_e232, 3i, 120i);
    let _e236: vec2<f32> = wq;
    let _e243: f32 = cloudFbm((_e236 + vec2<f32>(64f, 23f)), 3i, 121i);
    warp = (vec2<f32>(_e235, _e243) - vec2(0.5f));
    let _e249: vec2<f32> = q_15;
    let _e250: vec2<f32> = warp;
    let _e255: f32 = cloudFbm((_e249 + (_e250 * CLOUD_WARP)), 5i, 122i);
    return _e255;
}

fn daySkyAt(t_31: f32) -> vec3<f32> {
    var t_32: f32;
    var g_3: f32;
    var c_53: vec3<f32>;

    t_32 = t_31;
    let _e224: f32 = t_32;
    g_3 = smoothstep(0f, DAY_HORIZON_T, _e224);
    let _e227: VoxelScene = u;
    let _e230: VoxelScene = u;
    let _e235: f32 = g_3;
    c_53 = mix(_e227.dayA.xyz, _e230.dayB.xyz, vec3(smoothstep(0f, 0.55f, _e235)));
    let _e240: vec3<f32> = c_53;
    let _e241: VoxelScene = u;
    let _e246: f32 = g_3;
    return mix(_e240, _e241.dayC.xyz, vec3(smoothstep(0.45f, 1f, _e246)));
}

fn litSky(px_2: i32, py_6: i32) -> vec4<f32> {
    var px_3: i32;
    var py_7: i32;
    var t_33: f32;
    var gradient: bool;
    var local_90: vec3<f32>;
    var sky_1: vec3<f32>;
    var day: f32;
    var b_13: f32;
    var warm: vec3<f32>;
    var xs: f32;
    var glow_2: f32 = 0f;
    var st_1: vec4<f32>;
    var flash: f32;
    var nearStrike: f32 = 0f;
    var ringPx: f32;
    var dx_42: f32;
    var cover: f32;
    var storm: f32;
    var env: f32;
    var q_16: vec2<f32>;
    var n_5: f32;
    var thr: f32;
    var d_2: f32;
    var toSun: vec2<f32>;
    var n2_: f32;
    var edge: f32;
    var lit_6: f32;
    var body_6: vec3<f32>;
    var col_2: vec3<f32>;
    var sunward: f32;
    var dayCol: vec3<f32>;

    px_3 = px_2;
    py_7 = py_6;
    let _e225: i32 = py_7;
    let _e226: VoxelScene = u;
    py_7 = (_e225 + _e226.camera.x);
    let _e230: i32 = py_7;
    let _e232: VoxelScene = u;
    t_33 = clamp((f32(_e230) / f32(max(_e232.camera.y, 1i))), 0f, 1f);
    let _e243: VoxelScene = u;
    gradient = (_e243.roofK.w > 0.5f);
    let _e249: bool = gradient;
    if _e249 {
        let _e250: VoxelScene = u;
        let _e253: VoxelScene = u;
        let _e256: f32 = t_33;
        local_90 = mix(_e250.skyC.xyz, _e253.skyHorizon.xyz, vec3(_e256));
    } else {
        let _e259: VoxelScene = u;
        local_90 = _e259.skyC.xyz;
    }
    let _e263: vec3<f32> = local_90;
    sky_1 = _e263;
    let _e265: VoxelScene = u;
    day = _e265.dayA.w;
    let _e269: f32 = day;
    if (_e269 > 0f) {
        {
            let _e274: f32 = t_33;
            b_13 = smoothstep(0.04f, 0.56f, _e274);
            let _e277: VoxelScene = u;
            let _e280: VoxelScene = u;
            let _e285: f32 = b_13;
            let _e289: VoxelScene = u;
            let _e294: f32 = b_13;
            warm = mix(mix(_e277.bandA.xyz, _e280.bandB.xyz, vec3(smoothstep(0f, 0.5f, _e285))), _e289.bandC.xyz, vec3(smoothstep(0.5f, 1f, _e294)));
            let _e299: vec3<f32> = sky_1;
            let _e300: vec3<f32> = warm;
            let _e301: f32 = t_33;
            let _e302: vec3<f32> = daySkyAt(_e301);
            let _e305: f32 = day;
            let _e309: f32 = day;
            sky_1 = mix(_e299, mix(_e300, _e302, vec3(smoothstep(0f, 1f, _e305))), vec3(_e309));
        }
    }
    let _e312: bool = gradient;
    let _e313: VoxelScene = u;
    if (_e312 && (_e313.dayK.w > 0f)) {
        {
            let _e319: i32 = px_3;
            let _e321: i32 = W;
            let _e322: i32 = S;
            xs = (((f32(_e319) / f32(max((_e321 * _e322), 1i))) * 2f) - 1f);
            let _e333: vec3<f32> = sky_1;
            let _e334: f32 = t_33;
            let _e335: vec3<f32> = bandAt(_e334);
            let _e338: VoxelScene = u;
            let _e342: f32 = xs;
            sky_1 = (_e333 + (_e335 * (1f + ((0.4f * _e338.sunLean.w) * _e342))));
        }
    }
    let _e349: VoxelScene = u;
    if (_e349.cloudBody.w > 0.001f) {
        {
            let _e354: i32 = px_3;
            let _e355: i32 = py_7;
            let _e356: f32 = t_33;
            let _e357: vec4<f32> = starAt(_e354, _e355, _e356);
            let _e358: VoxelScene = u;
            let _e364: f32 = t_33;
            st_1 = (_e357 * (_e358.cloudBody.w * (1f - smoothstep(0.25f, 0.6f, _e364))));
            let _e370: vec3<f32> = sky_1;
            let _e371: vec4<f32> = st_1;
            sky_1 = (_e370 + (_e371.xyz * STAR_GAIN));
            let _e375: vec4<f32> = st_1;
            let _e377: VoxelScene = u;
            glow_2 = (_e375.w * _e377.dayB.w);
        }
    }
    let _e381: VoxelScene = u;
    flash = _e381.flashC.w;
    let _e387: f32 = flash;
    if (_e387 > 0f) {
        {
            let _e390: i32 = W;
            let _e391: i32 = S;
            ringPx = f32((_e390 * _e391));
            let _e395: i32 = px_3;
            let _e399: VoxelScene = u;
            let _e402: i32 = S;
            dx_42 = abs(((f32(_e395) + 0.5f) - (_e399.strikeK.x * f32(_e402))));
            let _e408: f32 = dx_42;
            let _e409: f32 = ringPx;
            let _e410: f32 = dx_42;
            let _e414: i32 = S;
            dx_42 = (min(_e408, (_e409 - _e410)) / (28f * f32(_e414)));
            let _e418: f32 = dx_42;
            let _e420: f32 = dx_42;
            nearStrike = exp((-(_e418) * _e420));
            let _e423: vec3<f32> = sky_1;
            let _e424: VoxelScene = u;
            let _e429: f32 = nearStrike;
            sky_1 = (_e423 + (_e424.flashC.xyz * (0.04f + (0.06f * _e429))));
        }
    }
    let _e434: VoxelScene = u;
    cover = _e434.cloudK.x;
    let _e438: f32 = cover;
    if (_e438 < 0.001f) {
        {
            let _e441: vec3<f32> = sky_1;
            let _e442: f32 = glow_2;
            return vec4<f32>(_e441.x, _e441.y, _e441.z, _e442);
        }
    }
    let _e447: VoxelScene = u;
    storm = _e447.cloudK.z;
    let _e454: f32 = storm;
    let _e458: f32 = storm;
    let _e460: f32 = t_33;
    env = (1f - smoothstep(mix(0.22f, 0.5f, _e454), mix(0.52f, 0.8f, _e458), _e460));
    let _e464: f32 = env;
    if (_e464 <= 0f) {
        {
            let _e467: vec3<f32> = sky_1;
            let _e468: f32 = glow_2;
            return vec4<f32>(_e467.x, _e467.y, _e467.z, _e468);
        }
    }
    let _e473: i32 = px_3;
    let _e477: i32 = S;
    let _e480: VoxelScene = u;
    let _e484: i32 = py_7;
    let _e488: i32 = S;
    q_16 = vec2<f32>((((f32(_e473) + 0.5f) / f32(_e477)) + _e480.cloudK.y), (((f32(_e484) + 0.5f) / f32(_e488)) * CLOUD_FLAT));
    let _e494: vec2<f32> = q_16;
    let _e495: f32 = cloudNoise(_e494);
    n_5 = _e495;
    let _e499: f32 = cover;
    let _e502: f32 = storm;
    thr = (mix(0.74f, 0.36f, _e499) - (0.16f * _e502));
    let _e506: f32 = thr;
    let _e507: f32 = thr;
    let _e510: f32 = n_5;
    let _e512: f32 = env;
    let _e516: f32 = cover;
    d_2 = ((smoothstep(_e506, (_e507 + 0.08f), _e510) * _e512) * smoothstep(0f, 0.2f, _e516));
    let _e520: f32 = d_2;
    if (_e520 <= 0f) {
        {
            let _e523: vec3<f32> = sky_1;
            let _e524: f32 = glow_2;
            return vec4<f32>(_e523.x, _e523.y, _e523.z, _e524);
        }
    }
    let _e529: VoxelScene = u;
    let _e532: VoxelScene = u;
    toSun = normalize(vec2<f32>(_e529.sunK.x, (-(max(_e532.sunK.y, 0.25f)) * CLOUD_FLAT)));
    let _e542: vec2<f32> = q_16;
    let _e543: vec2<f32> = toSun;
    let _e547: f32 = cloudNoise((_e542 + (_e543 * 2f)));
    n2_ = _e547;
    let _e549: f32 = n_5;
    let _e550: f32 = n2_;
    edge = clamp((((_e549 - _e550) * 10f) + 0.2f), 0f, 1f);
    let _e560: f32 = edge;
    let _e561: VoxelScene = u;
    lit_6 = (_e560 * _e561.cloudLit.w);
    let _e566: VoxelScene = u;
    let _e571: f32 = d_2;
    let _e574: f32 = lit_6;
    let _e581: f32 = storm;
    body_6 = ((_e566.cloudBody.xyz * (1.15f - ((0.45f * _e571) * (1f - _e574)))) * (1f - (0.5f * _e581)));
    let _e586: vec3<f32> = body_6;
    let _e587: VoxelScene = u;
    let _e590: f32 = lit_6;
    col_2 = (_e586 + (_e587.cloudLit.xyz * _e590));
    let _e594: f32 = day;
    if (_e594 > 0f) {
        {
            let _e597: f32 = edge;
            let _e603: f32 = cover;
            let _e605: f32 = cover;
            let _e613: f32 = storm;
            sunward = (clamp((((_e597 * 2f) + 0.3f) - ((0.4f * _e603) * _e605)), 0f, 1f) * (1f - (0.7f * _e613)));
            let _e618: VoxelScene = u;
            let _e621: VoxelScene = u;
            let _e624: f32 = sunward;
            let _e629: f32 = storm;
            dayCol = (mix(_e618.cloudShade.xyz, _e621.cloudDay.xyz, vec3(_e624)) * (1f - (0.72f * _e629)));
            let _e634: vec3<f32> = col_2;
            let _e635: vec3<f32> = dayCol;
            let _e636: f32 = day;
            col_2 = mix(_e634, _e635, vec3(_e636));
        }
    }
    let _e639: f32 = flash;
    if (_e639 > 0f) {
        {
            let _e642: vec3<f32> = col_2;
            let _e643: VoxelScene = u;
            let _e648: f32 = nearStrike;
            let _e654: f32 = d_2;
            col_2 = (_e642 + ((_e643.flashC.xyz * (0.12f + (0.5f * _e648))) * (0.4f + (0.6f * _e654))));
        }
    }
    let _e659: vec3<f32> = sky_1;
    let _e660: vec3<f32> = col_2;
    let _e661: f32 = d_2;
    let _e665: vec3<f32> = mix(_e659, _e660, vec3((_e661 * 0.92f)));
    let _e666: f32 = glow_2;
    let _e668: f32 = d_2;
    return vec4<f32>(_e665.x, _e665.y, _e665.z, (_e666 * (1f - _e668)));
}

fn shoreDist(c_54: vec3<i32>, p_21: vec3<f32>, brim: bool) -> f32 {
    var c_55: vec3<i32>;
    var p_22: vec3<f32>;
    var brim_1: bool;
    var fx_1: f32;
    var fz_1: f32;
    var d_3: f32 = 9f;
    var dz: i32 = -1i;
    var dx_43: i32;
    var nx: i32;
    var nz: i32;
    var local_91: f32;
    var local_92: f32;
    var ex: f32;
    var local_93: f32;
    var local_94: f32;
    var ez: f32;

    c_55 = c_54;
    p_22 = p_21;
    brim_1 = brim;
    let _e232: vec3<f32> = p_22;
    let _e234: vec3<i32> = c_55;
    fx_1 = (_e232.x - f32(_e234.x));
    let _e239: vec3<f32> = p_22;
    let _e241: vec3<i32> = c_55;
    fz_1 = (_e239.z - f32(_e241.z));
    loop {
        let _e251: i32 = dz;
        if !((_e251 <= 1i)) {
            break;
        }
        {
            dx_43 = -1i;
            loop {
                let _e261: i32 = dx_43;
                if !((_e261 <= 1i)) {
                    break;
                }
                {
                    let _e268: i32 = dx_43;
                    let _e271: i32 = dz;
                    if ((_e268 == 0i) && (_e271 == 0i)) {
                        {
                            continue;
                        }
                    }
                    let _e275: vec3<i32> = c_55;
                    let _e277: i32 = dx_43;
                    nx = (_e275.x + _e277);
                    let _e280: vec3<i32> = c_55;
                    let _e282: i32 = dz;
                    nz = (_e280.z + _e282);
                    let _e285: i32 = nz;
                    let _e288: i32 = nz;
                    let _e289: i32 = D;
                    if ((_e285 < 0i) || (_e288 >= _e289)) {
                        {
                            continue;
                        }
                    }
                    let _e292: i32 = nx;
                    let _e293: vec3<i32> = c_55;
                    let _e295: i32 = nz;
                    let _e296: bool = solidAt(_e292, _e293.y, _e295);
                    let _e298: bool = brim_1;
                    let _e299: i32 = nx;
                    let _e300: vec3<i32> = c_55;
                    let _e304: i32 = nz;
                    let _e305: bool = solidAt(_e299, (_e300.y + 1i), _e304);
                    if (!(_e296) && !((_e298 && _e305))) {
                        {
                            continue;
                        }
                    }
                    let _e309: i32 = dx_43;
                    if (_e309 < 0i) {
                        let _e312: f32 = fx_1;
                        local_92 = _e312;
                    } else {
                        let _e313: i32 = dx_43;
                        if (_e313 > 0i) {
                            let _e317: f32 = fx_1;
                            local_91 = (1f - _e317);
                        } else {
                            local_91 = 0f;
                        }
                        let _e321: f32 = local_91;
                        local_92 = _e321;
                    }
                    let _e323: f32 = local_92;
                    ex = _e323;
                    let _e325: i32 = dz;
                    if (_e325 < 0i) {
                        let _e328: f32 = fz_1;
                        local_94 = _e328;
                    } else {
                        let _e329: i32 = dz;
                        if (_e329 > 0i) {
                            let _e333: f32 = fz_1;
                            local_93 = (1f - _e333);
                        } else {
                            local_93 = 0f;
                        }
                        let _e337: f32 = local_93;
                        local_94 = _e337;
                    }
                    let _e339: f32 = local_94;
                    ez = _e339;
                    let _e341: f32 = d_3;
                    let _e342: f32 = ex;
                    let _e343: f32 = ez;
                    d_3 = min(_e341, length(vec2<f32>(_e342, _e343)));
                }
                continuing {
                    let _e265: i32 = dx_43;
                    dx_43 = (_e265 + 1i);
                }
            }
        }
        continuing {
            let _e255: i32 = dz;
            dz = (_e255 + 1i);
        }
    }
    let _e347: f32 = d_3;
    return _e347;
}

fn plungeDist(c_56: vec3<i32>, p_23: vec3<f32>, yp: i32) -> f32 {
    var c_57: vec3<i32>;
    var p_24: vec3<f32>;
    var yp_1: i32;
    var d_4: f32 = 9f;
    var dz_1: i32 = -1i;
    var dx_44: i32;
    var nx_1: i32;
    var nz_1: i32;

    c_57 = c_56;
    p_24 = p_23;
    yp_1 = yp;
    loop {
        let _e237: i32 = dz_1;
        if !((_e237 <= 1i)) {
            break;
        }
        {
            dx_44 = -1i;
            loop {
                let _e247: i32 = dx_44;
                if !((_e247 <= 1i)) {
                    break;
                }
                {
                    let _e254: vec3<i32> = c_57;
                    let _e256: i32 = dx_44;
                    nx_1 = (_e254.x + _e256);
                    let _e259: vec3<i32> = c_57;
                    let _e261: i32 = dz_1;
                    nz_1 = (_e259.z + _e261);
                    let _e264: i32 = nz_1;
                    let _e267: i32 = nz_1;
                    let _e268: i32 = D;
                    let _e271: i32 = nx_1;
                    let _e272: i32 = yp_1;
                    let _e273: i32 = nz_1;
                    let _e274: i32 = freeQ(_e271, _e272, _e273);
                    let _e278: i32 = nx_1;
                    let _e279: i32 = yp_1;
                    let _e280: i32 = nz_1;
                    let _e281: bool = fallingAt(_e278, _e279, _e280);
                    if ((((_e264 < 0i) || (_e267 >= _e268)) || (_e274 == 0i)) || !(_e281)) {
                        {
                            continue;
                        }
                    }
                    let _e284: f32 = d_4;
                    let _e285: vec3<f32> = p_24;
                    let _e287: i32 = nx_1;
                    let _e291: i32 = nz_1;
                    d_4 = min(_e284, length((_e285.xz - vec2<f32>((f32(_e287) + 0.5f), (f32(_e291) + 0.5f)))));
                }
                continuing {
                    let _e251: i32 = dx_44;
                    dx_44 = (_e251 + 1i);
                }
            }
        }
        continuing {
            let _e241: i32 = dz_1;
            dz_1 = (_e241 + 1i);
        }
    }
    let _e299: f32 = d_4;
    return _e299;
}

fn foamAt(c_58: vec3<i32>, p_25: vec3<f32>, fill_3: i32) -> f32 {
    var c_59: vec3<i32>;
    var p_26: vec3<f32>;
    var fill_4: i32;
    var brim_2: bool;
    var st_2: f32;
    var cover_1: f32 = 0f;
    var ds: f32;
    var t_34: f32;
    var w_13: f32;
    var lace: f32;
    var local_95: i32;
    var dp: f32;
    var churn: f32;

    c_59 = c_58;
    p_26 = p_25;
    fill_4 = fill_3;
    let _e232: i32 = fill_4;
    let _e233: i32 = S;
    brim_2 = (_e232 >= _e233);
    let _e236: VoxelScene = u;
    st_2 = _e236.clock.y;
    let _e242: vec3<i32> = c_59;
    let _e243: vec3<f32> = p_26;
    let _e244: bool = brim_2;
    let _e245: f32 = shoreDist(_e242, _e243, _e244);
    ds = _e245;
    let _e247: f32 = ds;
    if (_e247 < 0.58000004f) {
        {
            let _e254: f32 = st_2;
            t_34 = (_e254 / FOAM_STEPS);
            let _e257: vec3<f32> = p_26;
            let _e260: f32 = t_34;
            let _e262: f32 = vnoiseT(_e257.xz, 1f, _e260, 80i);
            w_13 = (FOAM_WIDTH * (FOAM_BREATHE + _e262));
            let _e268: vec3<f32> = p_26;
            let _e271: f32 = t_34;
            let _e273: f32 = vnoiseT(_e268.xz, 3f, _e271, 81i);
            lace = smoothstep(0.3f, 0.7f, _e273);
            let _e278: f32 = w_13;
            let _e280: f32 = w_13;
            let _e281: f32 = ds;
            let _e286: f32 = lace;
            cover_1 = ((1f - smoothstep((0.35f * _e278), _e280, _e281)) * mix(0.45f, 1f, _e286));
        }
    }
    let _e289: vec3<i32> = c_59;
    let _e290: vec3<f32> = p_26;
    let _e291: bool = brim_2;
    if _e291 {
        let _e292: vec3<i32> = c_59;
        local_95 = (_e292.y + 1i);
    } else {
        let _e296: vec3<i32> = c_59;
        local_95 = _e296.y;
    }
    let _e299: i32 = local_95;
    let _e302: f32 = plungeDist(_e289, _e290, (_e299 + 1i));
    dp = _e302;
    let _e304: f32 = dp;
    if (_e304 < PLUNGE_RADIUS) {
        {
            let _e306: vec3<f32> = p_26;
            let _e309: f32 = st_2;
            let _e312: f32 = vnoiseT(_e306.xz, 3f, (_e309 / PLUNGE_STEPS), 82i);
            churn = _e312;
            let _e314: f32 = cover_1;
            let _e317: f32 = dp;
            let _e322: f32 = churn;
            cover_1 = max(_e314, ((1f - smoothstep(0.25f, PLUNGE_RADIUS, _e317)) * mix(0.5f, 1f, _e322)));
        }
    }
    let _e326: f32 = cover_1;
    return _e326;
}

fn rainRing(w_14: vec2<f32>, copy: i32) -> f32 {
    var w_15: vec2<f32>;
    var copy_1: i32;
    var tt: f32;
    var slot_9: i32;
    var phase_2: f32;
    var cellsX: i32;
    var cellW: f32;
    var g_4: vec2<f32>;
    var cell_1: vec2<i32>;
    var h_12: u32;
    var chance: f32;
    var hv: f32;
    var margin: f32;
    var age_1: f32;
    var m_16: vec2<f32>;
    var at_2: vec2<f32>;
    var centre: vec2<f32>;
    var e_7: vec2<f32>;
    var l_6: f32;
    var r_21: f32;
    var local_96: vec2<f32>;
    var grad: vec2<f32>;
    var px_4: f32;
    var edge_1: f32;
    var fade_1: f32;

    w_15 = w_14;
    copy_1 = copy;
    let _e234: VoxelScene = u;
    let _e239: i32 = copy_1;
    tt = ((_e234.clock.y / RAIN_SLOT) + (0.5f * f32(_e239)));
    let _e244: f32 = tt;
    let _e248: i32 = wrapI(i32(floor(_e244)), 256i);
    slot_9 = _e248;
    let _e250: f32 = tt;
    phase_2 = fract(_e250);
    let _e254: i32 = W;
    cellsX = max(1i, i32(((f32(_e254) / RAIN_CELL) + 0.5f)));
    let _e262: i32 = W;
    let _e264: i32 = cellsX;
    cellW = (f32(_e262) / f32(_e264));
    let _e268: vec2<f32> = w_15;
    let _e270: f32 = cellW;
    let _e272: vec2<f32> = w_15;
    let _e277: i32 = copy_1;
    g_4 = (vec2<f32>((_e268.x / _e270), (_e272.y / RAIN_CELL)) + vec2((0.5f * f32(_e277))));
    let _e283: vec2<f32> = g_4;
    cell_1 = vec2<i32>(floor(_e283));
    let _e287: vec2<i32> = cell_1;
    let _e289: i32 = cellsX;
    let _e290: i32 = wrapI(_e287.x, _e289);
    let _e291: vec2<i32> = cell_1;
    let _e293: i32 = slot_9;
    let _e295: i32 = copy_1;
    let _e297: u32 = cellHash(_e290, _e291.y, _e293, (90i + _e295));
    h_12 = _e297;
    let _e299: VoxelScene = u;
    chance = min(_e299.rainK.w, 0.98f);
    let _e305: u32 = h_12;
    hv = (f32((_e305 & 65535u)) / 65535f);
    let _e312: f32 = hv;
    let _e313: f32 = chance;
    if (_e312 >= _e313) {
        {
            return 0f;
        }
    }
    let _e316: f32 = chance;
    let _e317: f32 = hv;
    margin = clamp(((_e316 - _e317) / 0.06f), 0f, 1f);
    let _e325: f32 = phase_2;
    let _e326: u32 = h_12;
    age_1 = ((_e325 - ((f32(((_e326 >> 16u) & 255u)) / 255f) * 0.3f)) / RING_LIFE);
    let _e342: f32 = age_1;
    let _e345: f32 = age_1;
    if ((_e342 <= 0f) || (_e345 >= 1f)) {
        {
            return 0f;
        }
    }
    let _e350: f32 = cellW;
    m_16 = vec2<f32>((RING_RADIUS / _e350), 0.3f);
    let _e357: u32 = h_12;
    let _e364: u32 = h_12;
    at_2 = (vec2<f32>(f32(((_e357 >> 24u) & 15u)), f32(((_e364 >> 28u) & 15u))) / vec2(15f));
    let _e376: vec2<f32> = m_16;
    let _e377: vec2<f32> = at_2;
    let _e380: vec2<f32> = m_16;
    centre = (_e376 + (_e377 * (vec2(1f) - (2f * _e380))));
    let _e387: vec2<f32> = g_4;
    let _e388: vec2<f32> = g_4;
    let _e391: vec2<f32> = centre;
    let _e393: f32 = cellW;
    e_7 = (((_e387 - floor(_e388)) - _e391) * vec2<f32>(_e393, RAIN_CELL));
    let _e397: vec2<f32> = e_7;
    l_6 = length(_e397);
    let _e402: f32 = age_1;
    let _e405: f32 = age_1;
    r_21 = (RING_RADIUS * (1f - ((1f - _e402) * (1f - _e405))));
    let _e411: f32 = l_6;
    if (_e411 > 0.0001f) {
        let _e414: vec2<f32> = e_7;
        let _e415: f32 = l_6;
        let _e418: i32 = S;
        let _e420: i32 = RISE;
        local_96 = ((_e414 / vec2(_e415)) / vec2<f32>(f32(_e418), f32(_e420)));
    } else {
        let _e425: i32 = S;
        local_96 = vec2<f32>((1f / f32(_e425)), 0f);
    }
    let _e431: vec2<f32> = local_96;
    grad = _e431;
    let _e433: f32 = l_6;
    let _e434: f32 = r_21;
    let _e437: vec2<f32> = grad;
    px_4 = (abs((_e433 - _e434)) / max(length(_e437), 0.000001f));
    let _e444: f32 = px_4;
    edge_1 = max((1f - (_e444 / 0.8f)), 0f);
    let _e453: f32 = age_1;
    let _e456: f32 = age_1;
    let _e460: f32 = age_1;
    fade_1 = ((smoothstep(0f, 0.12f, _e453) * (1f - _e456)) * (1f - _e460));
    let _e464: f32 = edge_1;
    let _e465: f32 = fade_1;
    let _e467: f32 = margin;
    return ((_e464 * _e465) * _e467);
}

fn undersideLit(c_60: vec3<i32>, v_62: vec4<u32>, hp: vec3<f32>) -> vec3<f32> {
    var c_61: vec3<i32>;
    var v_63: vec4<u32>;
    var hp_1: vec3<f32>;
    var body_7: vec3<f32>;

    c_61 = c_60;
    v_63 = v_62;
    hp_1 = hp;
    let _e236: vec4<u32> = v_63;
    let _e237: vec4<u32> = v_63;
    let _e238: i32 = matOf(_e237);
    let _e239: vec3<f32> = strataOf(_e238);
    let _e240: vec3<f32> = blockBody(_e236, _e239);
    body_7 = _e240;
    let _e242: vec3<f32> = body_7;
    let _e245: vec3<f32> = hp_1;
    let _e251: vec3<f32> = glowAt((_e245 - vec3<f32>(0f, 0.5f, 0f)));
    let _e252: vec3<f32> = ladderLight(0f, 0f, _e251);
    let _e254: vec3<i32> = c_61;
    let _e257: f32 = hazeAt(f32(_e254.z));
    let _e258: vec3<f32> = hazed((_e242 * _e252), _e257);
    return _e258;
}

fn reflectTexel(hp_2: vec3<f32>, c_62: vec3<i32>, axis_1: i32, dirX: f32) -> vec2<i32> {
    var hp_3: vec3<f32>;
    var c_63: vec3<i32>;
    var axis_2: i32;
    var dirX_1: f32;
    var dx_45: i32;
    var dy_30: i32;
    var local_97: i32;

    hp_3 = hp_2;
    c_63 = c_62;
    axis_2 = axis_1;
    dirX_1 = dirX;
    let _e238: vec3<f32> = hp_3;
    let _e241: i32 = S;
    let _e247: i32 = S;
    dx_45 = clamp(i32(floor((fract(_e238.x) * f32(_e241)))), 0i, (_e247 - 1i));
    let _e252: vec3<i32> = c_63;
    let _e257: vec3<f32> = hp_3;
    let _e260: i32 = S;
    let _e266: i32 = S;
    dy_30 = clamp(i32(floor(((f32((_e252.y + 1i)) - _e257.y) * f32(_e260)))), 0i, (_e266 - 1i));
    let _e271: i32 = axis_2;
    if (_e271 == 0i) {
        {
            let _e274: f32 = dirX_1;
            if (_e274 > 0f) {
                local_97 = 0i;
            } else {
                let _e278: i32 = S;
                local_97 = (_e278 - 1i);
            }
            let _e282: i32 = local_97;
            dx_45 = _e282;
        }
    }
    let _e283: i32 = axis_2;
    if (_e283 == 1i) {
        {
            let _e286: i32 = S;
            dy_30 = (_e286 - 1i);
        }
    }
    let _e289: i32 = S;
    let _e292: i32 = dy_30;
    let _e294: i32 = dx_45;
    return vec2<i32>(((_e289 - 1i) - _e292), _e294);
}

fn glyphFilled(base_11: i32, col_3: i32, row_15: i32) -> bool {
    var base_12: i32;
    var col_4: i32;
    var row_16: i32;

    base_12 = base_11;
    col_4 = col_3;
    row_16 = row_15;
    let _e236: i32 = col_4;
    let _e239: i32 = col_4;
    let _e240: i32 = S;
    let _e243: i32 = row_16;
    let _e247: i32 = row_16;
    let _e248: i32 = S;
    if ((((_e236 < 0i) || (_e239 >= _e240)) || (_e243 < 0i)) || (_e247 >= _e248)) {
        {
            return true;
        }
    }
    let _e252: i32 = col_4;
    let _e253: i32 = base_12;
    let _e254: i32 = row_16;
    let _e258: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e252, (_e253 + _e254)), 0i);
    return ((_e258.x >> 2u) != 63u);
}

fn glyphTexelAt(v_64: vec4<u32>, hp_4: vec3<f32>, c_64: vec3<i32>, axis_3: i32, dirX_2: f32) -> bool {
    var v_65: vec4<u32>;
    var hp_5: vec3<f32>;
    var c_65: vec3<i32>;
    var axis_4: i32;
    var dirX_3: f32;
    var t_35: vec2<i32>;
    var base_13: i32;
    var row_17: i32;
    var col_5: i32;

    v_65 = v_64;
    hp_5 = hp_4;
    c_65 = c_64;
    axis_4 = axis_3;
    dirX_3 = dirX_2;
    let _e240: vec3<f32> = hp_5;
    let _e241: vec3<i32> = c_65;
    let _e242: i32 = axis_4;
    let _e243: f32 = dirX_3;
    let _e244: vec2<i32> = reflectTexel(_e240, _e241, _e242, _e243);
    t_35 = _e244;
    let _e246: vec4<u32> = v_65;
    let _e247: i32 = partOf(_e246);
    let _e250: vec4<u32> = v_65;
    let _e251: i32 = glyphOf(_e250);
    let _e253: i32 = S;
    let _e254: i32 = RISE;
    base_13 = (((_e247 * 8i) + _e251) * (_e253 + _e254));
    let _e258: i32 = S;
    let _e261: vec2<i32> = t_35;
    row_17 = ((_e258 - 1i) - _e261.x);
    let _e265: vec2<i32> = t_35;
    col_5 = _e265.y;
    let _e268: i32 = base_13;
    let _e269: i32 = col_5;
    let _e270: i32 = row_17;
    let _e271: bool = glyphFilled(_e268, _e269, _e270);
    let _e272: i32 = base_13;
    let _e273: i32 = col_5;
    let _e276: i32 = row_17;
    let _e277: bool = glyphFilled(_e272, (_e273 - 1i), _e276);
    let _e279: i32 = base_13;
    let _e280: i32 = col_5;
    let _e283: i32 = row_17;
    let _e284: bool = glyphFilled(_e279, (_e280 + 1i), _e283);
    let _e286: i32 = base_13;
    let _e287: i32 = col_5;
    let _e288: i32 = row_17;
    let _e291: bool = glyphFilled(_e286, _e287, (_e288 - 1i));
    let _e293: i32 = base_13;
    let _e294: i32 = col_5;
    let _e295: i32 = row_17;
    let _e298: bool = glyphFilled(_e293, _e294, (_e295 + 1i));
    return ((((_e271 && _e277) && _e284) && _e291) && _e298);
}

fn reflectMarch(p_27: vec3<f32>, r_22: vec3<f32>, hit: ptr<function, vec3<i32>>, hp_6: ptr<function, vec3<f32>>, axis_5: ptr<function, i32>) -> bool {
    var p_28: vec3<f32>;
    var r_23: vec3<f32>;
    var c_66: vec3<i32>;
    var dir_1: vec3<i32>;
    var local_98: f32;
    var local_99: f32;
    var local_100: f32;
    var inv_1: vec3<f32>;
    var local_101: f32;
    var local_102: f32;
    var local_103: f32;
    var next_1: vec3<f32>;
    var cap_4: i32;
    var tIn: f32 = 0f;
    var i_12: i32 = 0i;
    var v_66: vec4<u32>;
    var local_104: i32;
    var local_105: i32;

    p_28 = p_27;
    r_23 = r_22;
    let _e237: vec3<f32> = p_28;
    let _e238: vec3<f32> = r_23;
    c_66 = vec3<i32>(floor((_e237 + (_e238 * 0.001f))));
    let _e245: vec3<f32> = r_23;
    dir_1 = vec3<i32>(sign(_e245));
    let _e249: vec3<f32> = r_23;
    if (abs(_e249.x) > 0.000001f) {
        let _e255: vec3<f32> = r_23;
        local_98 = (1f / abs(_e255.x));
    } else {
        local_98 = 1000000000000000000000000000000f;
    }
    let _e261: f32 = local_98;
    let _e262: vec3<f32> = r_23;
    if (abs(_e262.y) > 0.000001f) {
        let _e268: vec3<f32> = r_23;
        local_99 = (1f / abs(_e268.y));
    } else {
        local_99 = 1000000000000000000000000000000f;
    }
    let _e274: f32 = local_99;
    let _e275: vec3<f32> = r_23;
    if (abs(_e275.z) > 0.000001f) {
        let _e281: vec3<f32> = r_23;
        local_100 = (1f / abs(_e281.z));
    } else {
        local_100 = 1000000000000000000000000000000f;
    }
    let _e287: f32 = local_100;
    inv_1 = vec3<f32>(_e261, _e274, _e287);
    let _e290: vec3<i32> = dir_1;
    if (_e290.x > 0i) {
        let _e294: vec3<i32> = c_66;
        let _e299: vec3<f32> = p_28;
        local_101 = (f32((_e294.x + 1i)) - _e299.x);
    } else {
        let _e302: vec3<f32> = p_28;
        let _e304: vec3<i32> = c_66;
        local_101 = (_e302.x - f32(_e304.x));
    }
    let _e309: f32 = local_101;
    let _e310: vec3<f32> = inv_1;
    let _e313: vec3<i32> = dir_1;
    if (_e313.y > 0i) {
        let _e317: vec3<i32> = c_66;
        let _e322: vec3<f32> = p_28;
        local_102 = (f32((_e317.y + 1i)) - _e322.y);
    } else {
        let _e325: vec3<f32> = p_28;
        let _e327: vec3<i32> = c_66;
        local_102 = (_e325.y - f32(_e327.y));
    }
    let _e332: f32 = local_102;
    let _e333: vec3<f32> = inv_1;
    let _e336: vec3<i32> = dir_1;
    if (_e336.z > 0i) {
        let _e340: vec3<i32> = c_66;
        let _e345: vec3<f32> = p_28;
        local_103 = (f32((_e340.z + 1i)) - _e345.z);
    } else {
        let _e348: vec3<f32> = p_28;
        let _e350: vec3<i32> = c_66;
        local_103 = (_e348.z - f32(_e350.z));
    }
    let _e355: f32 = local_103;
    let _e356: vec3<f32> = inv_1;
    next_1 = vec3<f32>((_e309 * _e310.x), (_e332 * _e333.y), (_e355 * _e356.z));
    let _e361: VoxelScene = u;
    cap_4 = i32(_e361.waterL.w);
    (*axis_5) = -1i;
    let _e370: vec3<i32> = c_66;
    (*hit) = _e370;
    loop {
        let _e373: i32 = i_12;
        let _e374: i32 = cap_4;
        if !((_e373 < _e374)) {
            break;
        }
        {
            let _e380: vec3<i32> = c_66;
            let _e382: i32 = H;
            let _e384: vec3<i32> = c_66;
            let _e389: vec3<i32> = c_66;
            let _e394: vec3<i32> = c_66;
            let _e396: i32 = D;
            if ((((_e380.y >= _e382) || (_e384.y < 0i)) || (_e389.z < 0i)) || (_e394.z >= _e396)) {
                {
                    break;
                }
            }
            let _e399: i32 = i_12;
            if (_e399 > 0i) {
                {
                    let _e402: vec3<i32> = c_66;
                    let _e404: vec3<i32> = c_66;
                    let _e406: vec3<i32> = c_66;
                    let _e408: vec4<u32> = at(_e402.x, _e404.y, _e406.z);
                    v_66 = _e408;
                    let _e410: vec4<u32> = v_66;
                    let _e411: bool = solidV(_e410);
                    let _e412: vec4<u32> = v_66;
                    let _e413: i32 = partOf(_e412);
                    let _e414: bool = isBlockPart(_e413);
                    let _e415: vec4<u32> = v_66;
                    let _e416: vec3<f32> = p_28;
                    let _e417: vec3<f32> = r_23;
                    let _e418: f32 = tIn;
                    let _e421: vec3<i32> = c_66;
                    let _e422: i32 = (*axis_5);
                    let _e423: vec3<f32> = r_23;
                    let _e425: bool = glyphTexelAt(_e415, (_e416 + (_e417 * _e418)), _e421, _e422, _e423.x);
                    if (_e411 || (_e414 && _e425)) {
                        {
                            let _e428: vec3<i32> = c_66;
                            (*hit) = _e428;
                            let _e429: vec3<f32> = p_28;
                            let _e430: vec3<f32> = r_23;
                            let _e431: f32 = tIn;
                            (*hp_6) = (_e429 + (_e430 * _e431));
                            return true;
                        }
                    }
                }
            }
            let _e435: vec3<f32> = next_1;
            let _e437: vec3<f32> = next_1;
            let _e440: vec3<f32> = next_1;
            let _e442: vec3<f32> = next_1;
            if ((_e435.x < _e437.y) && (_e440.x < _e442.z)) {
                local_105 = 0i;
            } else {
                let _e447: vec3<f32> = next_1;
                let _e449: vec3<f32> = next_1;
                if (_e447.y < _e449.z) {
                    local_104 = 1i;
                } else {
                    local_104 = 2i;
                }
                let _e455: i32 = local_104;
                local_105 = _e455;
            }
            let _e457: i32 = local_105;
            (*axis_5) = _e457;
            let _e458: i32 = (*axis_5);
            if (_e458 == 0i) {
                {
                    let _e461: vec3<f32> = next_1;
                    tIn = _e461.x;
                    let _e464: vec3<i32> = c_66;
                    let _e466: vec3<i32> = dir_1;
                    c_66.x = (_e464.x + _e466.x);
                    let _e470: vec3<f32> = next_1;
                    let _e472: vec3<f32> = inv_1;
                    next_1.x = (_e470.x + _e472.x);
                }
            } else {
                let _e475: i32 = (*axis_5);
                if (_e475 == 1i) {
                    {
                        let _e478: vec3<f32> = next_1;
                        tIn = _e478.y;
                        let _e481: vec3<i32> = c_66;
                        let _e483: vec3<i32> = dir_1;
                        c_66.y = (_e481.y + _e483.y);
                        let _e487: vec3<f32> = next_1;
                        let _e489: vec3<f32> = inv_1;
                        next_1.y = (_e487.y + _e489.y);
                    }
                } else {
                    {
                        let _e492: vec3<f32> = next_1;
                        tIn = _e492.z;
                        let _e495: vec3<i32> = c_66;
                        let _e497: vec3<i32> = dir_1;
                        c_66.z = (_e495.z + _e497.z);
                        let _e501: vec3<f32> = next_1;
                        let _e503: vec3<f32> = inv_1;
                        next_1.z = (_e501.z + _e503.z);
                    }
                }
            }
        }
        continuing {
            let _e377: i32 = i_12;
            i_12 = (_e377 + 1i);
        }
    }
    let _e506: vec3<f32> = p_28;
    let _e507: vec3<f32> = r_23;
    let _e508: f32 = tIn;
    (*hp_6) = (_e506 + (_e507 * _e508));
    return false;
}

fn reflectionShade(found: bool, c_67: vec3<i32>, hp_7: vec3<f32>, axis_6: i32, r_24: vec3<f32>) -> vec3<f32> {
    var found_1: bool;
    var c_68: vec3<i32>;
    var hp_8: vec3<f32>;
    var axis_7: i32;
    var r_25: vec3<f32>;
    var row_18: i32;
    var col_6: i32;
    var v_67: vec4<u32>;
    var t_36: vec2<i32>;
    var dy_31: i32;
    var solid: bool;
    var base_14: vec3<f32>;
    var canopy_4: f32;
    var m_17: i32;
    var atlasY_1: i32;
    var q_17: u32;
    var pigment_2: vec4<f32>;
    var e_8: vec3<f32>;
    var emits: bool;
    var on_4: f32 = 1f;
    var local_106: f32;

    found_1 = found;
    c_68 = c_67;
    hp_8 = hp_7;
    axis_7 = axis_6;
    r_25 = r_24;
    let _e240: bool = found_1;
    if !(_e240) {
        {
            let _e242: i32 = BASE;
            let _e244: vec3<f32> = hp_8;
            let _e246: i32 = S;
            let _e250: vec3<f32> = hp_8;
            let _e252: i32 = RISE;
            row_18 = i32(floor(((f32(_e242) - (_e244.y * f32(_e246))) - (_e250.z * f32(_e252)))));
            let _e259: vec3<f32> = hp_8;
            let _e261: i32 = S;
            let _e266: i32 = W;
            let _e267: i32 = S;
            let _e271: i32 = wrapI(i32(floor((_e259.x * f32(_e261)))), max((_e266 * _e267), 1i));
            col_6 = _e271;
            let _e273: i32 = col_6;
            let _e274: i32 = row_18;
            let _e276: VoxelScene = u;
            let _e284: vec4<f32> = textureLoad(wgpu_backdropTex, vec2<i32>(_e273, clamp(_e274, 0i, (_e276.extent.w - 1i))), 0i);
            return _e284.xyz;
        }
    }
    let _e286: vec3<i32> = c_68;
    let _e288: vec3<i32> = c_68;
    let _e290: vec3<i32> = c_68;
    let _e292: vec4<u32> = at(_e286.x, _e288.y, _e290.z);
    v_67 = _e292;
    let _e294: vec3<f32> = hp_8;
    let _e295: vec3<i32> = c_68;
    let _e296: i32 = axis_7;
    let _e297: vec3<f32> = r_25;
    let _e299: vec2<i32> = reflectTexel(_e294, _e295, _e296, _e297.x);
    t_36 = _e299;
    let _e301: i32 = S;
    let _e304: vec2<i32> = t_36;
    dy_31 = ((_e301 - 1i) - _e304.x);
    let _e308: vec4<u32> = v_67;
    let _e309: bool = solidV(_e308);
    solid = _e309;
    let _e311: bool = solid;
    let _e312: i32 = axis_7;
    if (_e311 && (_e312 == 1i)) {
        {
            let _e316: vec3<i32> = c_68;
            let _e317: vec4<u32> = v_67;
            let _e318: vec3<f32> = hp_8;
            let _e319: vec3<f32> = undersideLit(_e316, _e317, _e318);
            return _e319;
        }
    }
    let _e322: bool = solid;
    if _e322 {
        {
            let _e323: vec4<u32> = v_67;
            let _e324: i32 = matOf(_e323);
            m_17 = _e324;
            let _e326: vec4<u32> = v_67;
            let _e327: i32 = m_17;
            let _e329: i32 = m_17;
            let _e333: vec3<i32> = c_68;
            let _e335: vec3<i32> = c_68;
            let _e337: vec3<i32> = c_68;
            let _e340: vec2<i32> = t_36;
            let _e342: i32 = dy_31;
            let _e343: vec3<f32> = faceBase(_e327, (2i * (_e329 - 1i)), _e333.x, _e335.y, _e337.z, 0i, _e340.y, _e342);
            let _e344: vec3<f32> = blockBody(_e326, _e343);
            base_14 = _e344;
            let _e345: vec3<i32> = c_68;
            let _e347: vec3<i32> = c_68;
            let _e351: vec3<i32> = c_68;
            let _e353: f32 = canopyAt(_e345.x, (_e347.z - 1i), _e351.y);
            canopy_4 = _e353;
        }
    } else {
        {
            let _e354: vec4<u32> = v_67;
            let _e355: i32 = partOf(_e354);
            let _e358: vec4<u32> = v_67;
            let _e359: i32 = glyphOf(_e358);
            let _e361: i32 = S;
            let _e362: i32 = RISE;
            let _e365: i32 = dy_31;
            atlasY_1 = ((((_e355 * 8i) + _e359) * (_e361 + _e362)) + _e365);
            let _e368: vec2<i32> = t_36;
            let _e370: i32 = atlasY_1;
            let _e373: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e368.y, _e370), 0i);
            q_17 = _e373.x;
            let _e376: u32 = q_17;
            let _e380: vec4<u32> = v_67;
            let _e385: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e376 & 3u)), i32(_e380.w)), 0i);
            pigment_2 = _e385;
            let _e387: vec4<u32> = v_67;
            let _e390: vec3<f32> = styleEmit(i32(_e387.w));
            e_8 = _e390;
            let _e392: u32 = q_17;
            let _e398: vec4<f32> = pigment_2;
            let _e400: bool = emitsWhole(_e398.w);
            emits = ((i32((_e392 >> 2u)) == TONE_EMIT) || _e400);
            let _e405: bool = emits;
            if _e405 {
                {
                    let _e406: vec3<i32> = c_68;
                    let _e408: vec3<i32> = c_68;
                    let _e410: vec3<i32> = c_68;
                    let _e416: bool = cellEmit(_e406.x, _e408.y, _e410.z, (&e_8), (&on_4));
                }
            }
            let _e417: bool = emits;
            let _e418: vec3<f32> = e_8;
            if (_e417 && any((_e418 > vec3(0f)))) {
                {
                    let _e424: vec3<f32> = e_8;
                    let _e425: vec3<i32> = c_68;
                    let _e428: f32 = hazeAt(f32(_e425.z));
                    let _e429: vec3<f32> = hazed(_e424, _e428);
                    return _e429;
                }
            }
            let _e430: vec4<f32> = pigment_2;
            base_14 = _e430.xyz;
            let _e432: vec3<i32> = c_68;
            let _e434: vec3<i32> = c_68;
            let _e436: vec3<i32> = c_68;
            let _e438: vec4<u32> = v_67;
            let _e440: vec2<i32> = t_36;
            let _e442: i32 = dy_31;
            let _e445: bool = plantTexel(_e432.x, _e434.y, _e436.z, _e438, false, _e440.y, _e442, (&base_14));
            if !(_e445) {
                {
                    let _e447: vec4<u32> = v_67;
                    let _e451: vec3<f32> = styleAt(i32(_e447.w), 1i);
                    base_14 = _e451;
                }
            }
            let _e452: vec4<u32> = v_67;
            let _e453: i32 = partOf(_e452);
            if (_e453 == ANIMAL_INTERIM) {
                let _e455: vec3<i32> = c_68;
                let _e457: vec3<i32> = c_68;
                let _e461: vec3<i32> = c_68;
                let _e463: f32 = canopyAt(_e455.x, (_e457.z - 1i), _e461.y);
                local_106 = _e463;
            } else {
                let _e464: vec4<u32> = v_67;
                let _e465: f32 = plantCanopy(_e464);
                local_106 = _e465;
            }
            let _e467: f32 = local_106;
            canopy_4 = _e467;
        }
    }
    let _e468: vec3<f32> = base_14;
    let _e469: vec3<i32> = c_68;
    let _e471: vec3<i32> = c_68;
    let _e473: vec3<i32> = c_68;
    let _e475: vec2<i32> = t_36;
    let _e477: i32 = dy_31;
    let _e478: f32 = canopy_4;
    let _e479: vec3<f32> = shadeFront(_e468, _e469.x, _e471.y, _e473.z, _e475.y, _e477, _e478);
    let _e480: vec3<i32> = c_68;
    let _e483: f32 = hazeAt(f32(_e480.z));
    let _e484: vec3<f32> = hazed(_e479, _e483);
    return _e484;
}

fn fallStreak(px_5: i32, py_8: i32, st_3: f32) -> bool {
    var px_6: i32;
    var py_9: i32;
    var st_4: f32;
    var h_13: u32;
    var local_107: i32;
    var period_1: i32;
    var len: i32;
    var speed: i32;
    var ph: i32;

    px_6 = px_5;
    py_9 = py_8;
    st_4 = st_3;
    let _e236: i32 = px_6;
    let _e237: i32 = W;
    let _e238: i32 = S;
    let _e240: i32 = wrapI(_e236, (_e237 * _e238));
    let _e244: u32 = cellHash(_e240, 0i, 0i, 50i);
    h_13 = _e244;
    let _e246: u32 = h_13;
    if ((_e246 % 3u) != 0u) {
        {
            return false;
        }
    }
    let _e252: i32 = S;
    if (_e252 >= 8i) {
        local_107 = 32i;
    } else {
        local_107 = 16i;
    }
    let _e258: i32 = local_107;
    period_1 = _e258;
    let _e261: i32 = S;
    let _e265: u32 = h_13;
    len = (max(2i, (_e261 / 3i)) + i32(((_e265 >> 8u) & 1u)));
    let _e275: i32 = S;
    speed = max(1i, (_e275 / 4i));
    let _e280: i32 = py_9;
    let _e281: f32 = st_4;
    let _e282: i32 = speed;
    let _e288: u32 = h_13;
    let _e294: i32 = period_1;
    let _e295: i32 = wrapI(((_e280 - i32(floor((_e281 * f32(_e282))))) + i32((_e288 >> 16u))), _e294);
    ph = _e295;
    let _e297: i32 = ph;
    let _e298: i32 = len;
    return (_e297 < _e298);
}

fn flowDebug(fl: vec3<f32>, top_12: bool, p_29: vec3<f32>) -> vec3<f32> {
    var fl_1: vec3<f32>;
    var top_13: bool;
    var p_30: vec3<f32>;
    var local_108: vec3<f32>;
    var q_18: f32;
    var a_22: f32;
    var hue_1: vec3<f32>;
    var c_69: vec3<f32>;
    var lp: vec2<f32>;
    var o_3: vec2<f32>;
    var d_5: vec2<f32>;
    var e_9: vec2<f32>;
    var pa: vec2<f32>;
    var ba: vec2<f32>;
    var t_37: f32;
    var local_109: vec3<f32>;

    fl_1 = fl;
    top_13 = top_12;
    p_30 = p_29;
    let _e236: vec3<f32> = fl_1;
    if (_e236.z > 0.5f) {
        {
            let _e240: i32 = PX;
            let _e241: i32 = PY;
            let _e242: VoxelScene = u;
            let _e245: bool = fallStreak(_e240, _e241, _e242.clock.y);
            if _e245 {
                local_108 = vec3(1f);
            } else {
                local_108 = vec3<f32>(0.35f, 0.05f, 0.5f);
            }
            let _e253: vec3<f32> = local_108;
            return _e253;
        }
    }
    let _e254: vec3<f32> = fl_1;
    q_18 = length(_e254.xy);
    let _e258: f32 = q_18;
    if (_e258 < 0.01f) {
        {
            return vec3<f32>(0.01f, 0.02f, 0.2f);
        }
    }
    let _e265: vec3<f32> = fl_1;
    let _e267: vec3<f32> = fl_1;
    a_22 = atan2(_e265.y, _e267.x);
    let _e273: f32 = a_22;
    let _e275: f32 = a_22;
    let _e279: f32 = a_22;
    hue_1 = (vec3(0.5f) + (0.5f * vec3<f32>(cos(_e273), cos((_e275 - 2.0944f)), cos((_e279 + 2.0944f)))));
    let _e288: vec3<f32> = hue_1;
    let _e291: f32 = q_18;
    c_69 = (_e288 * (0.25f + (0.75f * _e291)));
    let _e296: bool = top_13;
    let _e297: i32 = S;
    if (_e296 && (_e297 >= 8i)) {
        {
            let _e301: vec3<f32> = p_30;
            let _e304: i32 = S;
            let _e308: vec3<f32> = p_30;
            let _e312: i32 = RISE;
            lp = vec2<f32>((fract(_e301.x) * f32(_e304)), ((1f - fract(_e308.z)) * f32(_e312)));
            let _e318: i32 = S;
            let _e322: i32 = RISE;
            o_3 = vec2<f32>((0.5f * f32(_e318)), (0.5f * f32(_e322)));
            let _e327: vec3<f32> = fl_1;
            let _e329: i32 = S;
            let _e332: vec3<f32> = fl_1;
            let _e335: i32 = RISE;
            d_5 = normalize(vec2<f32>((_e327.x * f32(_e329)), (-(_e332.y) * f32(_e335))));
            let _e341: vec2<f32> = o_3;
            let _e342: vec2<f32> = d_5;
            let _e345: i32 = S;
            let _e349: i32 = RISE;
            let _e351: i32 = S;
            e_9 = (_e341 + (((_e342 * 0.45f) * f32(_e345)) * vec2<f32>(1f, (f32(_e349) / f32(_e351)))));
            let _e358: vec2<f32> = lp;
            let _e359: vec2<f32> = o_3;
            pa = (_e358 - _e359);
            let _e362: vec2<f32> = e_9;
            let _e363: vec2<f32> = o_3;
            ba = (_e362 - _e363);
            let _e366: vec2<f32> = pa;
            let _e367: vec2<f32> = ba;
            let _e369: vec2<f32> = ba;
            let _e370: vec2<f32> = ba;
            t_37 = clamp((dot(_e366, _e367) / max(dot(_e369, _e370), 0.0001f)), 0f, 1f);
            let _e379: vec2<f32> = pa;
            let _e380: vec2<f32> = ba;
            let _e381: f32 = t_37;
            if (length((_e379 - (_e380 * _e381))) < 0.75f) {
                {
                    let _e387: f32 = t_37;
                    if (_e387 > 0.6f) {
                        local_109 = vec3<f32>(1f, 1f, 0.3f);
                    } else {
                        local_109 = vec3(1f);
                    }
                    let _e397: vec3<f32> = local_109;
                    c_69 = _e397;
                }
            }
        }
    }
    let _e398: vec3<f32> = c_69;
    return _e398;
}

fn litWaterFinish(colour: vec3<f32>) -> vec3<f32> {
    var colour_1: vec3<f32>;
    var local_110: vec3<f32>;
    var zf_3: i32;
    var local_111: vec3<f32>;
    var y_81: i32;
    var z_93: i32;
    var top_14: bool;
    var x_91: i32;
    var fill_5: i32;
    var k_9: f32;
    var yf_3: f32;
    var xw: f32;
    var ys_1: f32;
    var local_112: vec3<f32>;
    var p_31: vec3<f32>;
    var local_113: i32;
    var local_114: vec3<i32>;
    var open: vec3<i32>;
    var local_115: f32;
    var canopy_5: f32;
    var fl_2: vec3<f32>;
    var local_116: f32;
    var sunOn_2: f32;
    var local_117: f32;
    var ndl_4: f32;
    var local_118: f32;
    var sun_6: f32;
    var local_119: vec3<f32>;
    var glow_3: vec3<f32>;
    var here: vec3<f32>;
    var ratio: vec3<f32>;
    var glint_1: f32;
    var n_6: vec3<f32>;
    var d_6: vec3<f32>;
    var cosi: f32;
    var f_6: f32;
    var r_26: vec3<f32>;
    var under: vec3<f32>;
    var hc: vec3<i32>;
    var hp_9: vec3<f32>;
    var axis_8: i32;
    var found_2: bool;
    var hz_5: f32;
    var surf: vec3<f32>;
    var froth: vec3<f32>;
    var ring: f32;
    var ringC: vec3<f32>;
    var foam: f32;
    var glintC: vec3<f32>;

    colour_1 = colour;
    let _e232: i32 = wEntry;
    if ((_e232 & ENTRY_FLAT) != 0i) {
        {
            let _e236: vec3<f32> = colour_1;
            let _e237: vec3<f32> = wScatter;
            let _e238: VoxelScene = u;
            let _e242: f32 = wHaze;
            let _e243: VoxelScene = u;
            colour_1 = (_e236 + ((_e237 * _e238.dayL.x) + (_e242 * _e243.hazeC.xyz)));
            let _e249: i32 = wEntry;
            if ((_e249 & ENTRY_FALLING) == 0i) {
                {
                    let _e253: vec3<f32> = colour_1;
                    return _e253;
                }
            }
            let _e254: VoxelScene = u;
            if (_e254.clock.z > 0.5f) {
                {
                    let _e259: i32 = PX;
                    let _e260: i32 = PY;
                    let _e261: VoxelScene = u;
                    let _e264: bool = fallStreak(_e259, _e260, _e261.clock.y);
                    if _e264 {
                        local_110 = vec3(1f);
                    } else {
                        local_110 = vec3<f32>(0.35f, 0.05f, 0.5f);
                    }
                    let _e272: vec3<f32> = local_110;
                    return _e272;
                }
            }
            let _e273: i32 = wEntry;
            zf_3 = ((_e273 >> 12u) & 4095i);
            let _e280: i32 = PX;
            let _e281: i32 = PY;
            let _e282: VoxelScene = u;
            let _e285: bool = fallStreak(_e280, _e281, _e282.clock.y);
            if _e285 {
                let _e286: VoxelScene = u;
                let _e289: VoxelScene = u;
                let _e293: i32 = zf_3;
                let _e295: f32 = hazeAt(f32(_e293));
                let _e296: vec3<f32> = hazed((_e286.waterSurfaceC.xyz * _e289.dayL.x), _e295);
                local_111 = _e296;
            } else {
                let _e297: vec3<f32> = colour_1;
                local_111 = _e297;
            }
            let _e299: vec3<f32> = local_111;
            return _e299;
        }
    }
    let _e300: i32 = wEntry;
    y_81 = (_e300 & 4095i);
    let _e304: i32 = wEntry;
    z_93 = ((_e304 >> 12u) & 4095i);
    let _e311: i32 = wEntry;
    top_14 = ((_e311 >> 24u) != 0i);
    let _e318: i32 = PX;
    let _e319: i32 = S;
    x_91 = (_e318 / _e319);
    let _e322: i32 = x_91;
    let _e323: i32 = y_81;
    let _e324: i32 = z_93;
    let _e325: i32 = fillPxAt(_e322, _e323, _e324);
    fill_5 = _e325;
    let _e327: i32 = RISE;
    let _e329: i32 = S;
    k_9 = (f32(_e327) / f32(_e329));
    let _e333: i32 = BASE;
    let _e334: i32 = z_93;
    let _e335: i32 = RISE;
    let _e338: i32 = PY;
    let _e345: i32 = S;
    yf_3 = ((f32((((_e333 - (_e334 * _e335)) - _e338) - 1i)) + 0.5f) / f32(_e345));
    let _e349: i32 = PX;
    let _e353: i32 = S;
    xw = ((f32(_e349) + 0.5f) / f32(_e353));
    let _e357: i32 = y_81;
    let _e359: i32 = fill_5;
    let _e361: i32 = S;
    ys_1 = (f32(_e357) + (f32(_e359) / f32(_e361)));
    let _e366: bool = top_14;
    if _e366 {
        let _e367: f32 = xw;
        let _e368: f32 = ys_1;
        let _e369: i32 = z_93;
        let _e371: f32 = yf_3;
        let _e372: f32 = ys_1;
        let _e374: f32 = k_9;
        local_112 = vec3<f32>(_e367, _e368, (f32(_e369) + clamp(((_e371 - _e372) / _e374), 0f, 1f)));
    } else {
        let _e381: f32 = xw;
        let _e382: f32 = yf_3;
        let _e383: i32 = z_93;
        local_112 = vec3<f32>(_e381, _e382, f32(_e383));
    }
    let _e387: vec3<f32> = local_112;
    p_31 = _e387;
    let _e389: bool = top_14;
    if _e389 {
        let _e390: i32 = x_91;
        let _e391: i32 = fill_5;
        let _e392: i32 = S;
        if (_e391 < _e392) {
            let _e394: i32 = y_81;
            local_113 = _e394;
        } else {
            let _e395: i32 = y_81;
            local_113 = (_e395 + 1i);
        }
        let _e399: i32 = local_113;
        let _e400: i32 = z_93;
        local_114 = vec3<i32>(_e390, _e399, _e400);
    } else {
        let _e402: i32 = x_91;
        let _e403: i32 = y_81;
        let _e404: i32 = z_93;
        local_114 = vec3<i32>(_e402, _e403, (_e404 - 1i));
    }
    let _e409: vec3<i32> = local_114;
    open = _e409;
    let _e411: bool = top_14;
    if _e411 {
        let _e412: i32 = x_91;
        let _e413: i32 = z_93;
        let _e414: i32 = y_81;
        let _e415: f32 = canopyAt(_e412, _e413, _e414);
        local_115 = _e415;
    } else {
        let _e416: i32 = x_91;
        let _e417: i32 = z_93;
        let _e420: i32 = y_81;
        let _e421: f32 = canopyAt(_e416, (_e417 - 1i), _e420);
        local_115 = _e421;
    }
    let _e423: f32 = local_115;
    canopy_5 = _e423;
    let _e425: i32 = x_91;
    let _e426: i32 = y_81;
    let _e427: i32 = z_93;
    let _e429: vec3<f32> = flowAt(vec3<i32>(_e425, _e426, _e427));
    fl_2 = _e429;
    let _e431: VoxelScene = u;
    if (_e431.clock.z > 0.5f) {
        {
            let _e436: vec3<f32> = fl_2;
            let _e437: bool = top_14;
            let _e438: vec3<f32> = p_31;
            let _e439: vec3<f32> = flowDebug(_e436, _e437, _e438);
            return _e439;
        }
    }
    let _e440: VoxelScene = u;
    if (_e440.sunK.y > 0f) {
        local_116 = 1f;
    } else {
        local_116 = 0f;
    }
    let _e448: f32 = local_116;
    sunOn_2 = _e448;
    let _e450: bool = top_14;
    if _e450 {
        let _e451: VoxelScene = u;
        local_117 = _e451.sunK.y;
    } else {
        let _e454: VoxelScene = u;
        local_117 = -(_e454.sunK.z);
    }
    let _e459: f32 = local_117;
    ndl_4 = _e459;
    let _e461: f32 = sunOn_2;
    let _e464: f32 = ndl_4;
    if ((_e461 > 0f) && (_e464 > 0f)) {
        let _e468: vec3<f32> = p_31;
        let _e469: vec3<i32> = open;
        let _e470: f32 = sunReaches(_e468, _e469);
        local_118 = _e470;
    } else {
        local_118 = 0f;
    }
    let _e473: f32 = local_118;
    sun_6 = _e473;
    let _e475: bool = top_14;
    if _e475 {
        let _e476: vec3<f32> = p_31;
        local_119 = (_e476 + vec3<f32>(0f, 0.5f, 0f));
    } else {
        let _e482: vec3<f32> = p_31;
        local_119 = (_e482 - vec3<f32>(0f, 0f, 0.5f));
    }
    let _e489: vec3<f32> = local_119;
    let _e490: vec3<f32> = glowAt(_e489);
    glow_3 = _e490;
    let _e492: vec3<i32> = open;
    let _e494: vec3<i32> = open;
    let _e496: vec3<i32> = open;
    let _e498: f32 = skyOpen(_e492.x, _e494.y, _e496.z);
    let _e499: f32 = canopy_5;
    let _e501: f32 = sun_6;
    let _e502: vec3<f32> = glow_3;
    let _e503: vec3<f32> = ladderLight((_e498 * _e499), _e501, _e502);
    here = _e503;
    let _e505: vec3<f32> = here;
    let _e506: f32 = sunOn_2;
    let _e507: vec3<f32> = ladderRef(_e506);
    ratio = (_e505 / max(_e507, vec3(0.0001f)));
    let _e513: vec3<f32> = colour_1;
    let _e514: vec3<f32> = wScatter;
    let _e515: vec3<f32> = ratio;
    let _e517: f32 = wHaze;
    let _e518: VoxelScene = u;
    colour_1 = (_e513 + ((_e514 * _e515) + (_e517 * _e518.hazeC.xyz)));
    let _e524: bool = top_14;
    if !(_e524) {
        {
            let _e526: vec3<f32> = colour_1;
            return _e526;
        }
    }
    let _e528: vec3<f32> = p_31;
    let _e530: vec3<f32> = fl_2;
    let _e534: vec3<f32> = rippleNormal(_e528.xz, _e530.xy, (&glint_1));
    n_6 = _e534;
    let _e537: f32 = k_9;
    d_6 = normalize(vec3<f32>(0f, -(_e537), 1f));
    let _e543: vec3<f32> = d_6;
    let _e544: vec3<f32> = n_6;
    cosi = clamp(-(dot(_e543, _e544)), 0f, 1f);
    let _e551: VoxelScene = u;
    let _e557: f32 = cosi;
    f_6 = clamp((_e551.waterL.y * (0.02f + (0.98f * pow((1f - _e557), 5f)))), 0f, 1f);
    let _e568: vec3<f32> = d_6;
    let _e569: vec3<f32> = n_6;
    r_26 = reflect(_e568, _e569);
    let _e573: vec3<f32> = r_26;
    r_26.y = max(_e573.y, 0.05f);
    let _e577: vec3<f32> = r_26;
    r_26 = normalize(_e577);
    if REFLECTIONS {
        {
            let _e580: f32 = f_6;
            let _e582: vec3<f32> = colour_1;
            under = ((1f - _e580) * _e582);
            let _e588: vec3<f32> = p_31;
            let _e589: vec3<f32> = r_26;
            let _e596: bool = reflectMarch(_e588, _e589, (&hc), (&hp_9), (&axis_8));
            found_2 = _e596;
            let _e598: vec3<f32> = under;
            let _e599: f32 = f_6;
            let _e600: bool = found_2;
            let _e601: vec3<i32> = hc;
            let _e602: vec3<f32> = hp_9;
            let _e603: i32 = axis_8;
            let _e604: vec3<f32> = r_26;
            let _e605: vec3<f32> = reflectionShade(_e600, _e601, _e602, _e603, _e604);
            colour_1 = (_e598 + (_e599 * _e605));
        }
    }
    let _e608: i32 = z_93;
    let _e612: f32 = hazeAt((f32(_e608) + 0.5f));
    hz_5 = _e612;
    let _e614: VoxelScene = u;
    let _e617: VoxelScene = u;
    surf = (_e614.waterSurfaceC.xyz * _e617.dayL.x);
    let _e622: vec3<f32> = colour_1;
    let _e623: vec3<f32> = surf;
    let _e624: f32 = hz_5;
    let _e625: vec3<f32> = hazed(_e623, _e624);
    froth = max(_e622, _e625);
    let _e628: VoxelScene = u;
    let _e633: VoxelScene = u;
    let _e639: vec3<i32> = open;
    let _e641: vec3<i32> = open;
    let _e643: vec3<i32> = open;
    let _e645: f32 = skyOpen(_e639.x, _e641.y, _e643.z);
    if (((_e628.waterM.z > 0f) && (_e633.rainK.w > 0f)) && (_e645 > 0f)) {
        {
            let _e649: vec3<f32> = p_31;
            let _e652: f32 = rainRing(_e649.xz, 0i);
            let _e653: vec3<f32> = p_31;
            let _e656: f32 = rainRing(_e653.xz, 1i);
            ring = max(_e652, _e656);
            let _e659: vec3<f32> = colour_1;
            let _e660: vec3<f32> = surf;
            let _e661: VoxelScene = u;
            let _e665: VoxelScene = u;
            let _e670: f32 = hz_5;
            let _e671: vec3<f32> = hazed(mix(_e660, vec3(_e661.dayL.x), vec3(_e665.waterM.w)), _e670);
            ringC = max(_e659, _e671);
            let _e674: vec3<f32> = colour_1;
            let _e675: vec3<f32> = ringC;
            let _e676: VoxelScene = u;
            let _e679: f32 = ring;
            colour_1 = mix(_e674, _e675, vec3(clamp((_e676.waterM.z * _e679), 0f, 1f)));
        }
    }
    let _e686: VoxelScene = u;
    if (_e686.waterM.x > 0f) {
        {
            let _e691: i32 = x_91;
            let _e692: i32 = y_81;
            let _e693: i32 = z_93;
            let _e695: vec3<f32> = p_31;
            let _e696: i32 = fill_5;
            let _e697: f32 = foamAt(vec3<i32>(_e691, _e692, _e693), _e695, _e696);
            foam = _e697;
            let _e699: vec3<f32> = colour_1;
            let _e700: vec3<f32> = froth;
            let _e701: VoxelScene = u;
            let _e704: f32 = foam;
            colour_1 = mix(_e699, _e700, vec3(clamp((_e701.waterM.x * _e704), 0f, 1f)));
        }
    }
    let _e711: VoxelScene = u;
    let _e716: f32 = glint_1;
    let _e720: f32 = sun_6;
    if (((_e711.waterM.y > 0f) && (_e716 > 0f)) && (_e720 > 0f)) {
        {
            let _e724: VoxelScene = u;
            let _e729: VoxelScene = u;
            let _e734: f32 = hz_5;
            let _e735: vec3<f32> = hazed(mix(_e724.sunLean.xyz, vec3(1f), vec3(_e729.waterM.w)), _e734);
            glintC = _e735;
            let _e737: vec3<f32> = colour_1;
            let _e738: vec3<f32> = glintC;
            let _e739: VoxelScene = u;
            let _e742: f32 = glint_1;
            let _e744: VoxelScene = u;
            colour_1 = mix(_e737, _e738, vec3(clamp(((_e739.waterM.y * _e742) * _e744.dayL.y), 0f, 1f)));
        }
    }
    let _e753: vec3<f32> = colour_1;
    return _e753;
}

fn wxTime() -> f32 {
    let _e230: VoxelScene = u;
    return _e230.cloudK.w;
}

fn wholeIn3600_(speed_1: f32, period_2: f32) -> f32 {
    var speed_2: f32;
    var period_3: f32;

    speed_2 = speed_1;
    period_3 = period_2;
    let _e235: f32 = speed_2;
    let _e238: f32 = period_3;
    let _e244: f32 = period_3;
    return ((max(1f, floor((((_e235 * 3600f) / _e238) + 0.5f))) * _e244) / 3600f);
}

fn fogShape(y_82: f32) -> f32 {
    var y_83: f32;
    var h_14: f32;
    var local_120: f32;

    y_83 = y_82;
    let _e232: f32 = y_83;
    let _e233: VoxelScene = u;
    let _e236: VoxelScene = u;
    h_14 = (_e232 - (_e233.fogK.y + _e236.fogK.z));
    let _e242: f32 = h_14;
    if (_e242 <= 0f) {
        local_120 = 1f;
    } else {
        let _e246: f32 = h_14;
        let _e248: VoxelScene = u;
        local_120 = exp((-(_e246) / max(_e248.fogK.w, 0.001f)));
    }
    let _e256: f32 = local_120;
    return _e256;
}

fn fogNoise(x_92: f32, z_94: f32) -> f32 {
    var x_93: f32;
    var z_95: f32;
    var mode: i32;
    var local_121: f32;
    var flow_4: f32;
    var w_16: vec2<f32>;
    var n_7: f32;
    var d_7: f32;
    var wb_1: vec2<f32>;
    var a_23: f32;
    var b_14: f32;
    var per: f32;
    var ph_1: f32;

    x_93 = x_92;
    z_95 = z_94;
    let _e237: VoxelScene = u;
    mode = i32((_e237.fogM.x + 0.5f));
    let _e244: i32 = mode;
    if (_e244 >= 2i) {
        let _e247: f32 = wxTime();
        let _e248: VoxelScene = u;
        let _e251: i32 = W;
        let _e253: f32 = wholeIn3600_(_e248.fogM.w, f32(_e251));
        let _e254: f32 = (_e247 * _e253);
        let _e255: i32 = W;
        let _e256: f32 = f32(_e255);
        local_121 = (_e254 - (floor((_e254 / _e256)) * _e256));
    } else {
        local_121 = 0f;
    }
    let _e263: f32 = local_121;
    flow_4 = _e263;
    let _e265: f32 = x_93;
    let _e266: VoxelScene = u;
    let _e270: f32 = flow_4;
    let _e272: f32 = z_95;
    w_16 = vec2<f32>(((_e265 - _e266.cloudK.y) - _e270), (_e272 * 1.6f));
    let _e278: vec2<f32> = w_16;
    let _e279: f32 = wxTime();
    let _e282: f32 = vnoiseT(_e278, FOG_FREQ, (_e279 * FOG_EVOLVE), 140i);
    let _e285: vec2<f32> = w_16;
    let _e291: f32 = vnoise(_e285, 0.25555554f, 7i, 141i);
    n_7 = ((0.65f * _e282) + (0.35f * _e291));
    let _e297: f32 = n_7;
    d_7 = (0.35f + (1.3f * _e297));
    let _e301: i32 = mode;
    if (_e301 >= 1i) {
        {
            let _e304: f32 = x_93;
            let _e305: f32 = flow_4;
            let _e307: f32 = z_95;
            wb_1 = vec2<f32>((_e304 - _e305), (_e307 * 1.6f));
            let _e313: vec2<f32> = wb_1;
            let _e316: f32 = vnoise(_e313, FOG_BANK_FREQ, 11i, 142i);
            a_23 = ((2f * _e316) - 1f);
            let _e322: vec2<f32> = wb_1;
            let _e325: f32 = vnoise(_e322, FOG_BANK_FREQ, 12i, 143i);
            b_14 = ((2f * _e325) - 1f);
            let _e333: VoxelScene = u;
            per = (3600f / max(1f, floor(((3600f / max(_e333.fogM.z, 1f)) + 0.5f))));
            let _e346: f32 = wxTime();
            let _e348: f32 = per;
            ph_1 = ((6.2831855f * _e346) / _e348);
            let _e351: f32 = d_7;
            let _e354: VoxelScene = u;
            let _e357: f32 = a_23;
            let _e358: f32 = ph_1;
            let _e361: f32 = b_14;
            let _e362: f32 = ph_1;
            d_7 = (_e351 * max(0f, (1f + (_e354.fogM.y * ((_e357 * cos(_e358)) + (_e361 * sin(_e362)))))));
        }
    }
    let _e370: f32 = d_7;
    return _e370;
}

fn fogSpan(y0_1: f32, k_10: f32, zEnd: f32) -> vec2<f32> {
    var y0_2: f32;
    var k_11: f32;
    var zEnd_1: f32;
    var top_15: f32;
    var zb: f32;

    y0_2 = y0_1;
    k_11 = k_10;
    zEnd_1 = zEnd;
    let _e239: VoxelScene = u;
    let _e242: VoxelScene = u;
    top_15 = (_e239.fogK.y + _e242.fogK.z);
    let _e247: f32 = y0_2;
    let _e248: f32 = top_15;
    let _e250: f32 = k_11;
    let _e253: f32 = zEnd_1;
    zb = clamp(((_e247 - _e248) / _e250), 0f, _e253);
    let _e256: f32 = zb;
    let _e257: f32 = zEnd_1;
    let _e258: f32 = zb;
    return vec2<f32>(_e256, (_e257 - _e258));
}

fn fogDepth(y0_3: f32, k_12: f32, zEnd_2: f32, noise: f32) -> f32 {
    var y0_4: f32;
    var k_13: f32;
    var zEnd_3: f32;
    var noise_1: f32;
    var span_2: vec2<f32>;
    var top_16: f32;
    var fw: f32;
    var local_122: f32;
    var above: f32;

    y0_4 = y0_3;
    k_13 = k_12;
    zEnd_3 = zEnd_2;
    noise_1 = noise;
    let _e241: f32 = y0_4;
    let _e242: f32 = k_13;
    let _e243: f32 = zEnd_3;
    let _e244: vec2<f32> = fogSpan(_e241, _e242, _e243);
    span_2 = _e244;
    let _e246: VoxelScene = u;
    let _e249: VoxelScene = u;
    top_16 = (_e246.fogK.y + _e249.fogK.z);
    let _e254: VoxelScene = u;
    fw = max(_e254.fogK.w, 0.001f);
    let _e260: f32 = y0_4;
    let _e261: f32 = top_16;
    if (_e260 > _e261) {
        let _e263: f32 = fw;
        let _e264: f32 = k_13;
        let _e266: f32 = y0_4;
        let _e267: f32 = k_13;
        let _e268: vec2<f32> = span_2;
        let _e272: f32 = top_16;
        let _e275: f32 = fw;
        let _e278: f32 = y0_4;
        let _e279: f32 = top_16;
        let _e282: f32 = fw;
        local_122 = ((_e263 / _e264) * (exp((-(((_e266 - (_e267 * _e268.x)) - _e272)) / _e275)) - exp((-((_e278 - _e279)) / _e282))));
    } else {
        local_122 = 0f;
    }
    let _e289: f32 = local_122;
    above = _e289;
    let _e291: VoxelScene = u;
    let _e294: f32 = noise_1;
    let _e296: f32 = above;
    let _e297: vec2<f32> = span_2;
    let _e302: f32 = k_13;
    let _e303: f32 = k_13;
    return (((_e291.fogK.x * _e294) * (_e296 + _e297.y)) * sqrt((1f + (_e302 * _e303))));
}

fn fogNoiseFor(xw_1: f32, y0_5: f32, k_14: f32, zEnd_4: f32) -> f32 {
    var xw_2: f32;
    var y0_6: f32;
    var k_15: f32;
    var zEnd_5: f32;
    var span_3: vec2<f32>;

    xw_2 = xw_1;
    y0_6 = y0_5;
    k_15 = k_14;
    zEnd_5 = zEnd_4;
    let _e241: f32 = y0_6;
    let _e242: f32 = k_15;
    let _e243: f32 = zEnd_5;
    let _e244: vec2<f32> = fogSpan(_e241, _e242, _e243);
    span_3 = _e244;
    let _e246: f32 = xw_2;
    let _e247: vec2<f32> = span_3;
    let _e250: vec2<f32> = span_3;
    let _e254: f32 = fogNoise(_e246, (_e247.x + (0.5f * _e250.y)));
    return _e254;
}

fn fogDepthDeep(xw_3: f32, y0_7: f32, k_16: f32, zEnd_6: f32) -> f32 {
    var xw_4: f32;
    var y0_8: f32;
    var k_17: f32;
    var zEnd_7: f32;
    var base_15: f32;
    var nb: f32;
    var top_17: f32;
    var zb_1: f32;
    var ne: f32;
    var band_1: f32;
    var fw_1: f32;
    var local_123: f32;
    var above_1: f32;

    xw_4 = xw_3;
    y0_8 = y0_7;
    k_17 = k_16;
    zEnd_7 = zEnd_6;
    let _e242: VoxelScene = u;
    let _e245: VoxelScene = u;
    base_15 = (_e242.fogK.y + _e245.fogK.z);
    let _e250: f32 = xw_4;
    let _e251: f32 = y0_8;
    let _e252: f32 = base_15;
    let _e254: f32 = k_17;
    let _e257: f32 = zEnd_7;
    let _e259: f32 = fogNoise(_e250, clamp(((_e251 - _e252) / _e254), 0f, _e257));
    nb = _e259;
    let _e261: f32 = base_15;
    let _e262: f32 = nb;
    top_17 = (_e261 + (FOG_BILLOW * (_e262 - 1f)));
    let _e268: f32 = y0_8;
    let _e269: f32 = top_17;
    let _e271: f32 = k_17;
    let _e274: f32 = zEnd_7;
    zb_1 = clamp(((_e268 - _e269) / _e271), 0f, _e274);
    let _e277: f32 = xw_4;
    let _e278: f32 = zEnd_7;
    let _e279: f32 = fogNoise(_e277, _e278);
    ne = _e279;
    let _e281: f32 = zEnd_7;
    let _e282: f32 = zb_1;
    band_1 = (_e281 - _e282);
    let _e285: VoxelScene = u;
    fw_1 = max(_e285.fogK.w, 0.001f);
    let _e291: f32 = y0_8;
    let _e292: f32 = top_17;
    if (_e291 > _e292) {
        let _e294: f32 = fw_1;
        let _e295: f32 = k_17;
        let _e297: f32 = y0_8;
        let _e298: f32 = k_17;
        let _e299: f32 = zb_1;
        let _e302: f32 = top_17;
        let _e305: f32 = fw_1;
        let _e308: f32 = y0_8;
        let _e309: f32 = top_17;
        let _e312: f32 = fw_1;
        local_123 = ((_e294 / _e295) * (exp((-(((_e297 - (_e298 * _e299)) - _e302)) / _e305)) - exp((-((_e308 - _e309)) / _e312))));
    } else {
        local_123 = 0f;
    }
    let _e319: f32 = local_123;
    above_1 = _e319;
    let _e321: VoxelScene = u;
    let _e324: f32 = nb;
    let _e325: f32 = above_1;
    let _e327: f32 = band_1;
    let _e331: f32 = ne;
    let _e334: f32 = band_1;
    let _e339: f32 = k_17;
    let _e340: f32 = k_17;
    return ((_e321.fogK.x * ((_e324 * (_e325 + (0.5f * _e327))) + ((_e331 * 0.5f) * _e334))) * sqrt((1f + (_e339 * _e340))));
}

fn gustAt(xv: f32) -> f32 {
    var xv_1: f32;
    var t_38: f32;
    var n1_: f32;
    var n2_1: f32;
    var a_24: f32;
    var b_15: f32;

    xv_1 = xv;
    let _e250: f32 = wxTime();
    t_38 = _e250;
    let _e253: i32 = W;
    n1_ = max(1f, floor(((f32(_e253) / 38f) + 0.5f)));
    let _e263: i32 = W;
    n2_1 = max(1f, floor(((f32(_e263) / 17f) + 0.5f)));
    let _e273: f32 = xv_1;
    let _e274: f32 = n1_;
    let _e276: i32 = W;
    let _e279: f32 = t_38;
    a_24 = sin((6.2831855f * (((_e273 * _e274) / f32(_e276)) - (_e279 * 0.05f))));
    let _e287: f32 = xv_1;
    let _e288: f32 = n2_1;
    let _e290: i32 = W;
    let _e293: f32 = t_38;
    b_15 = sin(((6.2831855f * (((_e287 * _e288) / f32(_e290)) - (_e293 * 0.11f))) + 1.7f));
    let _e305: f32 = a_24;
    let _e309: f32 = b_15;
    return mix(GUST_LOW, 1f, clamp(((0.5f + (0.35f * _e305)) + (0.25f * _e309)), 0f, 1f));
}

fn rainStreak(px_7: f32, py_10: f32, mode_1: i32, layer_2: i32) -> f32 {
    var px_8: f32;
    var py_11: f32;
    var mode_2: i32;
    var layer_3: i32;
    var Sf: f32;
    var local_124: vec3<f32> = RAIN_SPEED;
    var local_125: vec3<f32> = RAIN_LAYER_SPEED;
    var speed_3: f32;
    var local_126: vec3<f32> = RAIN_LEN;
    var local_127: vec3<f32> = RAIN_LAYER_SPEED;
    var len_1: f32;
    var period_4: f32;
    var ringPx_1: f32;
    var local_128: vec3<f32> = RAIN_LANE;
    var lanes: f32;
    var laneW: f32;
    var local_129: vec3<f32> = RAIN_SLANT;
    var xs_1: f32;
    var lf: f32;
    var lane: i32;
    var h_15: u32;
    var off: f32;
    var width: f32;
    var cx: f32;
    var s_3: f32;
    var yp_2: f32;
    var cell_2: f32;
    var hc_1: u32;
    var local_130: vec3<f32> = RAIN_FILL;
    var pos: f32;
    var l_7: f32;
    var cy: f32;

    px_8 = px_7;
    py_11 = py_10;
    mode_2 = mode_1;
    layer_3 = layer_2;
    let _e256: i32 = S;
    Sf = f32(_e256);
    let _e259: i32 = mode_2;
    let _e263: f32 = local_124[_e259];
    let _e264: i32 = layer_3;
    let _e268: f32 = local_125[_e264];
    speed_3 = (_e263 * _e268);
    let _e271: i32 = mode_2;
    let _e275: f32 = local_126[_e271];
    let _e277: i32 = layer_3;
    let _e281: f32 = local_127[_e277];
    len_1 = (_e275 * mix(1f, _e281, 0.6f));
    let _e286: f32 = len_1;
    period_4 = (_e286 / 0.4f);
    let _e290: i32 = W;
    let _e292: f32 = Sf;
    ringPx_1 = (f32(_e290) * _e292);
    let _e296: f32 = ringPx_1;
    let _e297: i32 = mode_2;
    let _e301: f32 = local_128[_e297];
    let _e302: f32 = Sf;
    lanes = max(1f, floor(((_e296 / (_e301 * _e302)) + 0.5f)));
    let _e310: f32 = ringPx_1;
    let _e311: f32 = lanes;
    laneW = (_e310 / _e311);
    let _e314: f32 = px_8;
    let _e317: i32 = mode_2;
    let _e321: f32 = local_129[_e317];
    let _e322: f32 = py_11;
    xs_1 = ((_e314 + 0.5f) - (_e321 * (_e322 + 0.5f)));
    let _e328: f32 = xs_1;
    let _e329: f32 = laneW;
    lf = floor((_e328 / _e329));
    let _e333: f32 = lf;
    let _e335: f32 = lanes;
    let _e337: i32 = wrapI(i32(_e333), i32(_e335));
    lane = _e337;
    let _e339: i32 = lane;
    let _e340: i32 = layer_3;
    let _e341: i32 = mode_2;
    let _e343: u32 = cellHash(_e339, _e340, _e341, 170i);
    h_15 = _e343;
    let _e347: u32 = h_15;
    let _e355: f32 = laneW;
    off = ((0.15f + ((0.7f * f32((_e347 & 255u))) / 255f)) * _e355);
    let _e359: f32 = Sf;
    width = max(1f, (_e359 / 10f));
    let _e365: f32 = width;
    let _e369: f32 = xs_1;
    let _e370: f32 = lf;
    let _e371: f32 = laneW;
    let _e374: f32 = off;
    cx = clamp((((0.5f * _e365) + 0.5f) - abs(((_e369 - (_e370 * _e371)) - _e374))), 0f, 1f);
    let _e382: f32 = cx;
    if (_e382 <= 0f) {
        {
            return 0f;
        }
    }
    let _e386: f32 = speed_3;
    let _e389: u32 = h_15;
    let _e401: f32 = period_4;
    let _e402: f32 = wholeIn3600_((_e386 * (0.85f + ((0.3f * f32(((_e389 >> 8u) & 255u))) / 255f))), _e401);
    s_3 = _e402;
    let _e404: f32 = py_11;
    let _e407: f32 = Sf;
    let _e409: f32 = wxTime();
    let _e410: f32 = s_3;
    let _e413: f32 = period_4;
    let _e414: u32 = h_15;
    yp_2 = ((((_e404 + 0.5f) / _e407) - (_e409 * _e410)) + ((_e413 * f32(((_e414 >> 16u) & 255u))) / 255f));
    let _e426: f32 = yp_2;
    let _e427: f32 = period_4;
    cell_2 = floor((_e426 / _e427));
    let _e431: i32 = lane;
    let _e432: f32 = cell_2;
    let _e434: i32 = layer_3;
    let _e437: i32 = mode_2;
    let _e440: u32 = cellHash(_e431, i32(_e432), ((_e434 * 3i) + _e437), 171i);
    hc_1 = _e440;
    let _e442: u32 = hc_1;
    let _e448: i32 = mode_2;
    let _e452: f32 = local_130[_e448];
    if ((f32((_e442 & 65535u)) / 65535f) >= _e452) {
        {
            return 0f;
        }
    }
    let _e455: f32 = yp_2;
    let _e456: f32 = cell_2;
    let _e457: f32 = period_4;
    pos = (_e455 - (_e456 * _e457));
    let _e461: f32 = len_1;
    let _e464: u32 = hc_1;
    l_7 = (_e461 * (0.75f + ((0.5f * f32(((_e464 >> 16u) & 255u))) / 255f)));
    let _e477: f32 = l_7;
    let _e478: f32 = pos;
    let _e480: f32 = Sf;
    let _e485: f32 = pos;
    let _e486: f32 = Sf;
    cy = (clamp(((_e477 - _e478) * _e480), 0f, 1f) * clamp(((_e485 * _e486) + 0.5f), 0f, 1f));
    let _e495: f32 = cx;
    let _e496: f32 = cy;
    let _e500: f32 = pos;
    let _e501: f32 = l_7;
    return ((_e495 * _e496) * (0.35f + (0.65f * clamp((_e500 / _e501), 0f, 1f))));
}

fn rainOpen(x_94: i32, y0_9: f32, k_18: f32, zl: f32) -> bool {
    var x_95: i32;
    var y0_10: f32;
    var k_19: f32;
    var zl_1: f32;
    var y_84: f32;
    var yi: i32;
    var zi: i32;

    x_95 = x_94;
    y0_10 = y0_9;
    k_19 = k_18;
    zl_1 = zl;
    let _e256: f32 = y0_10;
    let _e257: f32 = k_19;
    let _e258: f32 = zl_1;
    y_84 = (_e256 - (_e257 * _e258));
    let _e262: f32 = y_84;
    let _e263: i32 = H;
    if (_e262 >= f32(_e263)) {
        {
            return true;
        }
    }
    let _e267: f32 = y_84;
    if (_e267 < 0f) {
        {
            return false;
        }
    }
    let _e271: f32 = y_84;
    yi = i32(_e271);
    let _e274: f32 = zl_1;
    zi = i32(_e274);
    let _e277: i32 = x_95;
    let _e278: i32 = yi;
    let _e279: i32 = zi;
    let _e280: bool = solidAt(_e277, _e278, _e279);
    let _e282: i32 = x_95;
    let _e283: i32 = yi;
    let _e284: i32 = zi;
    let _e285: i32 = roofGap(_e282, _e283, _e284);
    return (!(_e280) && (_e285 == 0i));
}

fn splashAt(w_17: vec2<f32>, copy_2: i32) -> f32 {
    var w_18: vec2<f32>;
    var copy_3: i32;
    var tt_1: f32;
    var slot_10: f32;
    var phase_3: f32;
    var c_70: vec2<i32>;
    var h_16: u32;
    var chance_1: f32;
    var hv_1: f32;
    var age_2: f32;
    var at_3: vec2<f32>;
    var e_10: vec2<f32>;
    var d_8: f32;
    var r_27: f32;
    var ring_1: f32;
    var dot_: f32;
    var fade_2: f32;

    w_18 = w_17;
    copy_3 = copy_2;
    let _e255: f32 = wxTime();
    let _e258: i32 = copy_3;
    tt_1 = ((_e255 / SPLASH_SLOT) + (0.5f * f32(_e258)));
    let _e263: f32 = tt_1;
    slot_10 = floor(_e263);
    let _e266: f32 = tt_1;
    let _e267: f32 = slot_10;
    phase_3 = (_e266 - _e267);
    let _e270: vec2<f32> = w_18;
    let _e272: i32 = copy_3;
    c_70 = vec2<i32>(floor((_e270 + vec2((0.5f * f32(_e272))))));
    let _e280: vec2<i32> = c_70;
    let _e282: i32 = W;
    let _e283: i32 = wrapI(_e280.x, _e282);
    let _e284: vec2<i32> = c_70;
    let _e286: f32 = slot_10;
    let _e289: i32 = copy_3;
    let _e291: u32 = cellHash(_e283, _e284.y, i32(_e286), (180i + _e289));
    h_16 = _e291;
    let _e293: VoxelScene = u;
    chance_1 = dot(_e293.rainK.xyz, SPLASH_CHANCE);
    let _e298: u32 = h_16;
    hv_1 = (f32((_e298 & 65535u)) / 65535f);
    let _e305: f32 = hv_1;
    let _e306: f32 = chance_1;
    if (_e305 >= _e306) {
        {
            return 0f;
        }
    }
    let _e309: f32 = phase_3;
    let _e310: u32 = h_16;
    age_2 = ((_e309 - ((f32(((_e310 >> 16u) & 255u)) / 255f) * 0.64f)) / SPLASH_LIFE);
    let _e326: f32 = age_2;
    let _e329: f32 = age_2;
    if ((_e326 <= 0f) || (_e329 >= 1f)) {
        {
            return 0f;
        }
    }
    let _e337: u32 = h_16;
    let _e344: u32 = h_16;
    at_3 = (vec2(0.2f) + ((0.6f * vec2<f32>(f32(((_e337 >> 24u) & 15u)), f32(((_e344 >> 28u) & 15u)))) / vec2(15f)));
    let _e358: vec2<f32> = w_18;
    let _e360: i32 = copy_3;
    let _e365: vec2<i32> = c_70;
    let _e368: vec2<f32> = at_3;
    let _e370: i32 = S;
    let _e372: i32 = RISE;
    e_10 = ((((_e358 + vec2((0.5f * f32(_e360)))) - vec2<f32>(_e365)) - _e368) * vec2<f32>(f32(_e370), f32(_e372)));
    let _e377: vec2<f32> = e_10;
    d_8 = length(_e377);
    let _e382: f32 = age_2;
    let _e386: i32 = S;
    r_27 = ((0.08f + (0.22f * sqrt(_e382))) * f32(_e386));
    let _e391: f32 = d_8;
    let _e392: f32 = r_27;
    let _e401: f32 = age_2;
    ring_1 = (max((1f - (abs((_e391 - _e392)) / 0.9f)), 0f) * (1f - _e401));
    let _e406: f32 = d_8;
    let _e415: f32 = age_2;
    dot_ = (max((1f - (_e406 / 1.2f)), 0f) * (1f - smoothstep(0f, 0.35f, _e415)));
    let _e422: f32 = age_2;
    let _e424: f32 = chance_1;
    let _e425: f32 = hv_1;
    fade_2 = (smoothstep(0f, 0.08f, _e422) * clamp(((_e424 - _e425) / 0.08f), 0f, 1f));
    let _e434: f32 = ring_1;
    let _e435: f32 = dot_;
    let _e437: f32 = fade_2;
    return (max(_e434, _e435) * _e437);
}

fn segDist(p_32: vec2<f32>, a_25: vec2<f32>, b_16: vec2<f32>) -> f32 {
    var p_33: vec2<f32>;
    var a_26: vec2<f32>;
    var b_17: vec2<f32>;
    var pa_1: vec2<f32>;
    var ba_1: vec2<f32>;
    var t_39: f32;

    p_33 = p_32;
    a_26 = a_25;
    b_17 = b_16;
    let _e261: vec2<f32> = p_33;
    let _e262: vec2<f32> = a_26;
    pa_1 = (_e261 - _e262);
    let _e265: vec2<f32> = b_17;
    let _e266: vec2<f32> = a_26;
    ba_1 = (_e265 - _e266);
    let _e269: vec2<f32> = pa_1;
    let _e270: vec2<f32> = ba_1;
    let _e272: vec2<f32> = ba_1;
    let _e273: vec2<f32> = ba_1;
    t_39 = clamp((dot(_e269, _e270) / max(dot(_e272, _e273), 0.0001f)), 0f, 1f);
    let _e282: vec2<f32> = pa_1;
    let _e283: vec2<f32> = ba_1;
    let _e284: f32 = t_39;
    return length((_e282 - (_e283 * _e284)));
}

fn boltHash(i_13: i32, salt_18: i32) -> f32 {
    var i_14: i32;
    var salt_19: i32;
    var h_17: u32;

    i_14 = i_13;
    salt_19 = salt_18;
    let _e259: VoxelScene = u;
    let _e263: VoxelScene = u;
    let _e267: i32 = i_14;
    let _e268: i32 = salt_19;
    let _e269: u32 = cellHash(i32(_e259.strikeL.x), i32(_e263.strikeL.y), _e267, _e268);
    h_17 = _e269;
    let _e271: u32 = h_17;
    return ((f32((_e271 & 65535u)) / 65535f) - 0.5f);
}

fn kinkDist(p_34: vec2<f32>, a_27: vec2<f32>, b_18: vec2<f32>, kink: f32) -> f32 {
    var p_35: vec2<f32>;
    var a_28: vec2<f32>;
    var b_19: vec2<f32>;
    var kink_1: f32;
    var m_18: vec2<f32>;

    p_35 = p_34;
    a_28 = a_27;
    b_19 = b_18;
    kink_1 = kink;
    let _e264: vec2<f32> = a_28;
    let _e265: vec2<f32> = b_19;
    let _e268: f32 = kink_1;
    m_18 = ((0.5f * (_e264 + _e265)) + vec2<f32>(_e268, 0f));
    let _e273: vec2<f32> = p_35;
    let _e274: vec2<f32> = a_28;
    let _e275: vec2<f32> = m_18;
    let _e276: f32 = segDist(_e273, _e274, _e275);
    let _e277: vec2<f32> = p_35;
    let _e278: vec2<f32> = m_18;
    let _e279: vec2<f32> = b_19;
    let _e280: f32 = segDist(_e277, _e278, _e279);
    return min(_e276, _e280);
}

fn boltAt(p_36: vec2<f32>) -> vec3<f32> {
    var p_37: vec2<f32>;
    var Sf_1: f32;
    var end: vec2<f32>;
    var ringPx_2: f32;
    var top_18: vec2<f32>;
    var segH: f32;
    var swing: f32;
    var drift: f32 = 0f;
    var j_1: i32 = 1i;
    var forkAt0_: i32;
    var forkAt1_: i32;
    var fork0_: vec2<f32>;
    var fork1_: vec2<f32>;
    var d_9: f32 = 1000000000f;
    var a_29: vec2<f32>;
    var walk_2: f32 = 0f;
    var i_15: i32 = 1i;
    var f_7: f32;
    var y_85: f32;
    var b_20: vec2<f32>;
    var fd: f32 = 1000000000f;
    var f_8: i32 = 0i;
    var local_131: f32;
    var side: f32;
    var local_132: vec2<f32>;
    var fa: vec2<f32>;
    var j_2: i32;
    var fb: vec2<f32>;
    var w_19: f32;
    var core: f32;
    var glow_4: f32;
    var coreC: vec3<f32> = vec3<f32>(0.92f, 0.9f, 1f);
    var glowC: vec3<f32>;

    p_37 = p_36;
    let _e257: i32 = S;
    Sf_1 = f32(_e257);
    let _e260: VoxelScene = u;
    let _e263: f32 = Sf_1;
    let _e265: i32 = BASE;
    let _e267: VoxelScene = u;
    let _e270: i32 = RISE;
    let _e274: VoxelScene = u;
    let _e279: f32 = Sf_1;
    let _e283: i32 = RISE;
    end = vec2<f32>((_e260.strikeK.x * _e263), (((f32(_e265) - (_e267.strikeK.z * f32(_e270))) - ((_e274.strikeK.y + 1f) * _e279)) - (0.5f * f32(_e283))));
    let _e289: i32 = W;
    let _e291: f32 = Sf_1;
    ringPx_2 = (f32(_e289) * _e291);
    let _e295: vec2<f32> = end;
    let _e297: f32 = ringPx_2;
    let _e298: vec2<f32> = p_37;
    let _e300: vec2<f32> = end;
    let _e303: f32 = ringPx_2;
    end.x = (_e295.x + (_e297 * floor((((_e298.x - _e300.x) / _e303) + 0.5f))));
    let _e310: vec2<f32> = p_37;
    let _e312: vec2<f32> = end;
    let _e317: f32 = Sf_1;
    let _e320: vec2<f32> = p_37;
    let _e322: vec2<f32> = end;
    let _e325: f32 = Sf_1;
    if ((abs((_e310.x - _e312.x)) > (24f * _e317)) || (_e320.y > (_e322.y + (2f * _e325)))) {
        {
            return vec3(0f);
        }
    }
    let _e332: vec2<f32> = end;
    let _e336: f32 = boltHash(0i, 192i);
    let _e339: f32 = Sf_1;
    let _e344: VoxelScene = u;
    top_18 = vec2<f32>((_e332.x + ((_e336 * 8f) * _e339)), (-0.03f * f32(_e344.extent.w)));
    let _e351: vec2<f32> = end;
    let _e353: vec2<f32> = top_18;
    segH = ((_e351.y - _e353.y) / 18f);
    let _e360: f32 = segH;
    swing = ((BOLT_SWING * _e360) * 2f);
    loop {
        let _e369: i32 = j_1;
        if !((_e369 <= BOLT_SEGS)) {
            break;
        }
        {
            let _e375: f32 = drift;
            let _e376: i32 = j_1;
            let _e378: f32 = boltHash(_e376, 190i);
            let _e379: f32 = swing;
            drift = (_e375 + (_e378 * _e379));
        }
        continuing {
            let _e372: i32 = j_1;
            j_1 = (_e372 + 1i);
        }
    }
    let _e385: f32 = boltHash(0i, 193i);
    forkAt0_ = (3i + i32(((_e385 + 0.5f) * 5f)));
    let _e393: i32 = forkAt0_;
    let _e398: f32 = boltHash(1i, 193i);
    forkAt1_ = ((_e393 + 3i) + i32(((_e398 + 0.5f) * 5f)));
    let _e406: vec2<f32> = top_18;
    fork0_ = _e406;
    let _e408: vec2<f32> = top_18;
    fork1_ = _e408;
    let _e412: vec2<f32> = top_18;
    a_29 = _e412;
    loop {
        let _e418: i32 = i_15;
        if !((_e418 <= BOLT_SEGS)) {
            break;
        }
        {
            let _e424: f32 = walk_2;
            let _e425: i32 = i_15;
            let _e427: f32 = boltHash(_e425, 190i);
            let _e428: f32 = swing;
            walk_2 = (_e424 + (_e427 * _e428));
            let _e431: i32 = i_15;
            f_7 = (f32(_e431) / 18f);
            let _e437: vec2<f32> = top_18;
            let _e439: f32 = segH;
            let _e440: i32 = i_15;
            y_85 = (_e437.y + (_e439 * f32(_e440)));
            let _e445: i32 = i_15;
            if (_e445 < BOLT_SEGS) {
                {
                    let _e447: f32 = y_85;
                    let _e448: i32 = i_15;
                    let _e450: f32 = boltHash(_e448, 191i);
                    let _e453: f32 = segH;
                    y_85 = (_e447 + ((_e450 * 0.5f) * _e453));
                }
            }
            let _e456: vec2<f32> = top_18;
            let _e458: f32 = walk_2;
            let _e460: vec2<f32> = end;
            let _e462: vec2<f32> = top_18;
            let _e465: f32 = drift;
            let _e467: f32 = f_7;
            let _e470: f32 = y_85;
            b_20 = vec2<f32>(((_e456.x + _e458) + (((_e460.x - _e462.x) - _e465) * _e467)), _e470);
            let _e473: vec2<f32> = p_37;
            let _e476: vec2<f32> = a_29;
            let _e478: vec2<f32> = b_20;
            let _e484: f32 = segH;
            let _e486: f32 = Sf_1;
            if (abs((_e473.y - (0.5f * (_e476.y + _e478.y)))) < (_e484 + (2f * _e486))) {
                {
                    let _e490: f32 = d_9;
                    let _e491: vec2<f32> = p_37;
                    let _e492: vec2<f32> = a_29;
                    let _e493: vec2<f32> = b_20;
                    let _e494: i32 = i_15;
                    let _e496: f32 = boltHash(_e494, 197i);
                    let _e500: f32 = segH;
                    let _e502: f32 = kinkDist(_e491, _e492, _e493, (((_e496 * 2f) * BOLT_KINK) * _e500));
                    d_9 = min(_e490, _e502);
                }
            }
            let _e504: i32 = i_15;
            let _e505: i32 = forkAt0_;
            if (_e504 == _e505) {
                {
                    let _e507: vec2<f32> = b_20;
                    fork0_ = _e507;
                }
            }
            let _e508: i32 = i_15;
            let _e509: i32 = forkAt1_;
            if (_e508 == _e509) {
                {
                    let _e511: vec2<f32> = b_20;
                    fork1_ = _e511;
                }
            }
            let _e512: vec2<f32> = b_20;
            a_29 = _e512;
        }
        continuing {
            let _e421: i32 = i_15;
            i_15 = (_e421 + 1i);
        }
    }
    loop {
        let _e517: i32 = f_8;
        if !((_e517 < 2i)) {
            break;
        }
        {
            let _e524: i32 = f_8;
            let _e526: f32 = boltHash(_e524, 194i);
            if (_e526 < 0f) {
                local_131 = -1f;
            } else {
                local_131 = 1f;
            }
            let _e533: f32 = local_131;
            side = _e533;
            let _e535: i32 = f_8;
            if (_e535 == 0i) {
                let _e538: vec2<f32> = fork0_;
                local_132 = _e538;
            } else {
                let _e539: vec2<f32> = fork1_;
                local_132 = _e539;
            }
            let _e541: vec2<f32> = local_132;
            fa = _e541;
            j_2 = 1i;
            loop {
                let _e545: i32 = j_2;
                if !((_e545 <= FORK_SEGS)) {
                    break;
                }
                {
                    let _e551: vec2<f32> = fa;
                    let _e552: f32 = side;
                    let _e556: i32 = f_8;
                    let _e558: i32 = j_2;
                    let _e561: f32 = boltHash(((10i * _e556) + _e558), 195i);
                    let _e569: f32 = segH;
                    let _e571: f32 = segH;
                    let _e575: i32 = f_8;
                    let _e577: i32 = j_2;
                    let _e580: f32 = boltHash(((10i * _e575) + _e577), 196i);
                    fb = (_e551 + vec2<f32>((((_e552 * (0.3f + (0.9f * (_e561 + 0.5f)))) * 0.6f) * _e569), (_e571 * (0.45f + (0.4f * (_e580 + 0.5f))))));
                    let _e589: f32 = fd;
                    let _e590: vec2<f32> = p_37;
                    let _e591: vec2<f32> = fa;
                    let _e592: vec2<f32> = fb;
                    let _e594: i32 = f_8;
                    let _e596: i32 = j_2;
                    let _e599: f32 = boltHash(((10i * _e594) + _e596), 198i);
                    let _e602: f32 = segH;
                    let _e604: f32 = kinkDist(_e590, _e591, _e592, ((_e599 * 0.3f) * _e602));
                    let _e605: i32 = j_2;
                    fd = min(_e589, (_e604 + (f32(_e605) * 0.4f)));
                    let _e611: vec2<f32> = fb;
                    fa = _e611;
                }
                continuing {
                    let _e548: i32 = j_2;
                    j_2 = (_e548 + 1i);
                }
            }
        }
        continuing {
            let _e521: i32 = f_8;
            f_8 = (_e521 + 1i);
        }
    }
    let _e613: f32 = Sf_1;
    w_19 = max(0.75f, (_e613 / 9f));
    let _e618: f32 = w_19;
    let _e621: f32 = d_9;
    let _e628: f32 = w_19;
    let _e632: f32 = fd;
    core = (clamp(((_e618 + 0.5f) - _e621), 0f, 1f) + (0.55f * clamp((((0.5f * _e628) + 0.5f) - _e632), 0f, 1f)));
    let _e640: f32 = d_9;
    let _e643: f32 = Sf_1;
    let _e648: f32 = fd;
    let _e651: f32 = Sf_1;
    glow_4 = (exp((-(_e640) / (0.9f * _e643))) + (0.4f * exp((-(_e648) / (0.6f * _e651)))));
    let _e663: VoxelScene = u;
    glowC = mix(_e663.lightC.xyz, vec3<f32>(0.62f, 0.42f, 1f), vec3(0.6f));
    let _e674: VoxelScene = u;
    let _e678: f32 = core;
    let _e680: vec3<f32> = coreC;
    let _e683: f32 = glow_4;
    let _e685: vec3<f32> = glowC;
    return (_e674.strikeK.w * (((2.2f * _e678) * _e680) + ((0.6f * _e683) * _e685)));
}

fn over(c_71: ptr<function, vec3<f32>>, a_30: ptr<function, f32>, lc_1: vec3<f32>, la: f32) {
    var lc_2: vec3<f32>;
    var la_1: f32;

    lc_2 = lc_1;
    la_1 = la;
    let _e261: vec3<f32> = (*c_71);
    let _e263: f32 = la_1;
    let _e266: vec3<f32> = lc_2;
    let _e267: f32 = la_1;
    (*c_71) = ((_e261 * (1f - _e263)) + (_e266 * _e267));
    let _e270: f32 = (*a_30);
    let _e271: f32 = la_1;
    let _e273: f32 = (*a_30);
    (*a_30) = (_e270 + (_e271 * (1f - _e273)));
    return;
}

fn weatherOver(px_9: vec2<i32>) -> vec4<f32> {
    var px_10: vec2<i32>;
    var zEnd_8: f32;
    var k_20: f32;
    var xw_5: f32;
    var y0_11: f32;
    var x_96: i32;
    var c_72: vec3<f32> = vec3(0f);
    var a_31: f32 = 0f;
    var local_133: f32;
    var t_40: f32;
    var fl_3: vec3<f32>;
    var zg: f32;
    var wsum: f32;
    var depth_4: f32;
    var local_134: f32;
    var gust: f32;
    var curtain: vec3<f32> = vec3(0f);
    var dark_1: f32 = 0f;
    var bg: vec3<f32>;
    var Y: vec3<f32> = vec3<f32>(0.2126f, 0.7152f, 0.0722f);
    var veil: f32;
    var zi_1: i32;
    var q_19: i32;
    var local_135: i32;
    var level_4: i32;
    var r_28: i32;
    var w_20: vec2<f32>;
    var sp_4: f32;
    var sc: vec3<f32>;
    var l_8: i32 = 2i;
    var local_136: vec3<f32> = RAIN_DEPTH;
    var zl_2: f32;
    var rc: vec3<f32>;
    var m_19: i32;
    var wm: f32;
    var st_5: f32;
    var local_137: f32;
    var g_5: f32;
    var local_138: vec3<f32> = RAIN_ALPHA;
    var local_139: vec3<f32> = RAIN_LAYER_ALPHA;
    var cover_2: f32;

    px_10 = px_9;
    let _e257: vec2<i32> = px_10;
    let _e259: vec4<f32> = textureLoad(wgpu_depthTex, _e257, 0i);
    zEnd_8 = floor(((_e259.w * 255f) + 0.5f));
    let _e267: i32 = RISE;
    let _e269: i32 = S;
    k_20 = (f32(_e267) / f32(_e269));
    let _e273: i32 = PX;
    let _e277: i32 = S;
    xw_5 = ((f32(_e273) + 0.5f) / f32(_e277));
    let _e281: i32 = BASE;
    let _e282: vec2<i32> = px_10;
    let _e290: i32 = S;
    y0_11 = ((f32(((_e281 - _e282.y) - 1i)) + 0.5f) / f32(_e290));
    let _e294: i32 = PX;
    let _e295: i32 = S;
    x_96 = (_e294 / _e295);
    let _e304: VoxelScene = u;
    if (!(VOLUMETRIC) && (_e304.fogK.x > 0f)) {
        {
            let _e310: VoxelScene = u;
            if (i32((_e310.fogM.x + 0.5f)) == 3i) {
                let _e318: f32 = xw_5;
                let _e319: f32 = y0_11;
                let _e320: f32 = k_20;
                let _e321: f32 = zEnd_8;
                let _e322: f32 = fogDepthDeep(_e318, _e319, _e320, _e321);
                local_133 = exp(-(_e322));
            } else {
                let _e325: f32 = y0_11;
                let _e326: f32 = k_20;
                let _e327: f32 = zEnd_8;
                let _e328: f32 = xw_5;
                let _e329: f32 = y0_11;
                let _e330: f32 = k_20;
                let _e331: f32 = zEnd_8;
                let _e332: f32 = fogNoiseFor(_e328, _e329, _e330, _e331);
                let _e333: f32 = fogDepth(_e325, _e326, _e327, _e332);
                local_133 = exp(-(_e333));
            }
            let _e337: f32 = local_133;
            t_40 = _e337;
            let _e339: f32 = t_40;
            if (_e339 < 0.999f) {
                {
                    let _e342: VoxelScene = u;
                    let _e345: VoxelScene = u;
                    let _e348: VoxelScene = u;
                    let _e352: VoxelScene = u;
                    fl_3 = (_e342.fogC.xyz + ((_e345.rainC.w * _e348.dayL.y) * _e352.sunLean.xyz));
                    let _e358: VoxelScene = u;
                    if (GLOW && (_e358.fogC.w > 0f)) {
                        {
                            let _e364: f32 = zEnd_8;
                            zg = max((_e364 - 1.5f), 0.5f);
                            let _e370: vec3<f32> = fl_3;
                            let _e371: VoxelScene = u;
                            let _e374: f32 = xw_5;
                            let _e375: f32 = y0_11;
                            let _e376: f32 = k_20;
                            let _e377: f32 = zg;
                            let _e382: f32 = zg;
                            let _e384: vec3<f32> = glowAt(vec3<f32>(_e374, max((_e375 - (_e376 * _e377)), 0.5f), _e382));
                            fl_3 = (_e370 + (_e371.fogC.w * _e384));
                        }
                    }
                    let _e389: vec3<f32> = fl_3;
                    let _e391: f32 = t_40;
                    over((&c_72), (&a_31), _e389, (1f - _e391));
                }
            }
        }
    }
    let _e395: VoxelScene = u;
    let _e398: VoxelScene = u;
    let _e402: VoxelScene = u;
    wsum = ((_e395.rainK.x + _e398.rainK.y) + _e402.rainK.z);
    let _e407: f32 = wsum;
    let _e410: VoxelScene = u;
    let _e413: VoxelScene = u;
    let _e417: VoxelScene = u;
    if ((_e407 > 0f) || (((_e410.rainH.x + _e413.rainH.y) + _e417.rainH.z) > 0f)) {
        {
            let _e424: f32 = zEnd_8;
            let _e425: i32 = D;
            let _e428: i32 = D;
            depth_4 = (min(_e424, f32(_e425)) / f32(max(_e428, 1i)));
            let _e434: VoxelScene = u;
            let _e439: VoxelScene = u;
            if ((_e434.rainK.z > 0f) || (_e439.rainH.z > 0f)) {
                let _e445: f32 = xw_5;
                let _e447: f32 = y0_11;
                let _e450: f32 = gustAt((_e445 - (0.24f * _e447)));
                local_134 = _e450;
            } else {
                local_134 = 1f;
            }
            let _e453: f32 = local_134;
            gust = _e453;
            let _e460: f32 = zEnd_8;
            let _e461: i32 = D;
            if (_e460 >= f32(_e461)) {
                {
                    let _e464: vec2<i32> = px_10;
                    let _e466: vec4<f32> = textureLoad(wgpu_backdropTex, _e464, 0i);
                    bg = _e466.xyz;
                    let _e476: VoxelScene = u;
                    let _e479: vec3<f32> = Y;
                    let _e481: vec3<f32> = bg;
                    let _e482: vec3<f32> = Y;
                    dark_1 = (1f - smoothstep(0f, RAIN_CURTAIN_RAMP, (dot(_e476.rainC.xyz, _e479) - dot(_e481, _e482))));
                    let _e487: vec3<f32> = bg;
                    curtain = (_e487 * RAIN_CURTAIN);
                }
            }
            let _e489: VoxelScene = u;
            let _e494: f32 = gust;
            let _e498: f32 = depth_4;
            veil = (dot(_e489.rainH.xyz, (RAIN_VEIL * vec3<f32>(1f, 1f, _e494))) * _e498);
            let _e501: f32 = veil;
            if (_e501 > 0f) {
                {
                    let _e506: VoxelScene = u;
                    let _e510: vec3<f32> = hazed(_e506.rainC.xyz, 0.5f);
                    let _e511: vec3<f32> = curtain;
                    let _e512: f32 = dark_1;
                    let _e515: f32 = veil;
                    over((&c_72), (&a_31), mix(_e510, _e511, vec3(_e512)), _e515);
                }
            }
            let _e518: f32 = zEnd_8;
            zi_1 = i32(_e518);
            let _e521: i32 = zi_1;
            let _e522: i32 = D;
            if (_e521 < _e522) {
                {
                    let _e524: i32 = BASE;
                    let _e525: i32 = zi_1;
                    let _e526: i32 = RISE;
                    let _e529: vec2<i32> = px_10;
                    q_19 = (((_e524 - (_e525 * _e526)) - _e529.y) - 1i);
                    let _e535: i32 = q_19;
                    if (_e535 >= 0i) {
                        let _e538: i32 = q_19;
                        let _e539: i32 = S;
                        local_135 = (_e538 / _e539);
                    } else {
                        local_135 = -1i;
                    }
                    let _e544: i32 = local_135;
                    level_4 = _e544;
                    let _e546: i32 = q_19;
                    let _e547: i32 = level_4;
                    let _e548: i32 = S;
                    r_28 = (_e546 - (_e547 * _e548));
                    let _e552: i32 = level_4;
                    let _e555: i32 = r_28;
                    let _e556: i32 = RISE;
                    let _e559: i32 = level_4;
                    let _e562: i32 = H;
                    let _e565: i32 = x_96;
                    let _e566: i32 = level_4;
                    let _e569: i32 = zi_1;
                    let _e570: bool = solidAt(_e565, (_e566 - 1i), _e569);
                    let _e572: i32 = level_4;
                    let _e573: i32 = H;
                    let _e575: i32 = x_96;
                    let _e576: i32 = level_4;
                    let _e577: i32 = zi_1;
                    let _e578: bool = solidAt(_e575, _e576, _e577);
                    let _e582: i32 = x_96;
                    let _e583: i32 = level_4;
                    let _e586: i32 = zi_1;
                    let _e587: i32 = roofGap(_e582, (_e583 - 1i), _e586);
                    if ((((((_e552 >= 1i) && (_e555 < _e556)) && ((_e559 - 1i) < _e562)) && _e570) && !(((_e572 < _e573) && _e578))) && (_e587 == 0i)) {
                        {
                            let _e591: f32 = xw_5;
                            let _e592: i32 = zi_1;
                            let _e596: i32 = RISE;
                            let _e599: i32 = r_28;
                            let _e604: i32 = RISE;
                            w_20 = vec2<f32>(_e591, ((f32(_e592) + 1f) - ((f32(((_e596 - 1i) - _e599)) + 0.5f) / f32(_e604))));
                            let _e610: vec2<f32> = w_20;
                            let _e612: f32 = splashAt(_e610, 0i);
                            let _e613: vec2<f32> = w_20;
                            let _e615: f32 = splashAt(_e613, 1i);
                            sp_4 = max(_e612, _e615);
                            let _e618: f32 = sp_4;
                            if (_e618 > 0f) {
                                {
                                    let _e621: VoxelScene = u;
                                    let _e624: VoxelScene = u;
                                    let _e631: f32 = zEnd_8;
                                    let _e632: f32 = hazeAt(_e631);
                                    let _e633: vec3<f32> = hazed(mix(_e621.rainC.xyz, vec3(_e624.dayL.x), vec3(0.15f)), _e632);
                                    sc = _e633;
                                    let _e637: vec3<f32> = sc;
                                    let _e639: f32 = sp_4;
                                    over((&c_72), (&a_31), _e637, (0.5f * _e639));
                                }
                            }
                        }
                    }
                }
            }
            loop {
                let _e645: i32 = l_8;
                if !((_e645 >= 0i)) {
                    break;
                }
                {
                    let _e652: i32 = l_8;
                    let _e656: f32 = local_136[_e652];
                    let _e657: i32 = D;
                    zl_2 = (_e656 * f32(_e657));
                    let _e661: f32 = zEnd_8;
                    let _e662: f32 = zl_2;
                    let _e664: i32 = x_96;
                    let _e665: f32 = y0_11;
                    let _e666: f32 = k_20;
                    let _e667: f32 = zl_2;
                    let _e668: bool = rainOpen(_e664, _e665, _e666, _e667);
                    if ((_e661 <= _e662) || !(_e668)) {
                        {
                            continue;
                        }
                    }
                    let _e671: VoxelScene = u;
                    let _e674: f32 = zl_2;
                    let _e675: f32 = hazeAt(_e674);
                    let _e676: vec3<f32> = hazed(_e671.rainC.xyz, _e675);
                    let _e677: vec3<f32> = curtain;
                    let _e678: f32 = dark_1;
                    rc = mix(_e676, _e677, vec3(_e678));
                    m_19 = 0i;
                    loop {
                        let _e684: i32 = m_19;
                        if !((_e684 < 3i)) {
                            break;
                        }
                        {
                            let _e691: i32 = m_19;
                            let _e694: f32 = u.rainK[_e691];
                            wm = _e694;
                            let _e696: f32 = wm;
                            if (_e696 <= 0f) {
                                {
                                    continue;
                                }
                            }
                            let _e699: vec2<i32> = px_10;
                            let _e702: vec2<i32> = px_10;
                            let _e705: i32 = m_19;
                            let _e706: i32 = l_8;
                            let _e707: f32 = rainStreak(f32(_e699.x), f32(_e702.y), _e705, _e706);
                            st_5 = _e707;
                            let _e709: f32 = st_5;
                            if (_e709 <= 0f) {
                                {
                                    continue;
                                }
                            }
                            let _e712: i32 = m_19;
                            if (_e712 == 2i) {
                                let _e715: f32 = gust;
                                local_137 = _e715;
                            } else {
                                local_137 = 1f;
                            }
                            let _e718: f32 = local_137;
                            g_5 = _e718;
                            let _e720: f32 = st_5;
                            let _e721: f32 = wm;
                            let _e723: f32 = g_5;
                            let _e725: i32 = m_19;
                            let _e729: f32 = local_138[_e725];
                            let _e731: i32 = l_8;
                            let _e735: f32 = local_139[_e731];
                            cover_2 = ((((_e720 * _e721) * _e723) * _e729) * _e735);
                            let _e740: vec3<f32> = rc;
                            let _e741: f32 = cover_2;
                            let _e743: f32 = dark_1;
                            over((&c_72), (&a_31), _e740, clamp((_e741 * mix(1f, RAIN_CURTAIN_ALPHA, _e743)), 0f, 1f));
                        }
                        continuing {
                            let _e688: i32 = m_19;
                            m_19 = (_e688 + 1i);
                        }
                    }
                }
                continuing {
                    let _e649: i32 = l_8;
                    l_8 = (_e649 - 1i);
                }
            }
        }
    }
    let _e751: VoxelScene = u;
    let _e756: f32 = zEnd_8;
    let _e757: VoxelScene = u;
    if ((_e751.strikeK.w > 0f) && (_e756 >= _e757.strikeK.z)) {
        {
            let _e762: vec3<f32> = c_72;
            let _e763: vec2<i32> = px_10;
            let _e768: vec3<f32> = boltAt((vec2<f32>(_e763) + vec2(0.5f)));
            c_72 = (_e762 + _e768);
        }
    }
    let _e770: vec3<f32> = c_72;
    let _e771: f32 = a_31;
    return vec4<f32>(_e770.x, _e770.y, _e770.z, _e771);
}

fn flatRain(px_11: i32, py_12: i32, colour_2: vec3<f32>) -> vec3<f32> {
    var px_12: i32;
    var py_13: i32;
    var colour_3: vec3<f32>;
    var wsum_1: f32;
    var local_140: i32;
    var local_141: i32;
    var m_20: i32;
    var st_6: f32;
    var local_142: vec3<f32> = RAIN_ALPHA;

    px_12 = px_11;
    py_13 = py_12;
    colour_3 = colour_2;
    let _e261: VoxelScene = u;
    let _e264: VoxelScene = u;
    let _e268: VoxelScene = u;
    wsum_1 = ((_e261.rainK.x + _e264.rainK.y) + _e268.rainK.z);
    let _e273: VoxelScene = u;
    let _e276: VoxelScene = u;
    let _e279: VoxelScene = u;
    if (_e273.rainK.z >= max(_e276.rainK.x, _e279.rainK.y)) {
        local_141 = 2i;
    } else {
        let _e285: VoxelScene = u;
        let _e288: VoxelScene = u;
        if (_e285.rainK.y >= _e288.rainK.x) {
            local_140 = 1i;
        } else {
            local_140 = 0i;
        }
        let _e295: i32 = local_140;
        local_141 = _e295;
    }
    let _e297: i32 = local_141;
    m_20 = _e297;
    let _e299: i32 = px_12;
    let _e301: i32 = py_13;
    let _e303: i32 = m_20;
    let _e305: f32 = rainStreak(f32(_e299), f32(_e301), _e303, 0i);
    st_6 = _e305;
    let _e307: f32 = st_6;
    if (_e307 <= 0f) {
        {
            let _e310: vec3<f32> = colour_3;
            return _e310;
        }
    }
    let _e311: vec3<f32> = colour_3;
    let _e312: VoxelScene = u;
    let _e315: f32 = st_6;
    let _e316: f32 = wsum_1;
    let _e320: i32 = m_20;
    let _e324: f32 = local_142[_e320];
    return mix(_e311, _e312.rainC.xyz, vec3(clamp((((_e315 * min(_e316, 1f)) * _e324) * 0.8f), 0f, 1f)));
}

fn sunInAir(p_38: vec3<f32>) -> f32 {
    var p_39: vec3<f32>;
    var v_68: vec2<f32>;

    p_39 = p_38;
    let _e258: vec3<f32> = p_39;
    let _e259: i32 = W;
    let _e261: i32 = H;
    let _e263: i32 = D;
    let _e268: vec4<f32> = textureSampleLevel(wgpu_sunVisTex, wgpu_sampler_0_33_, (_e258 / vec3<f32>(f32(_e259), f32(_e261), f32(_e263))), 0f);
    v_68 = _e268.xy;
    let _e271: vec2<f32> = v_68;
    let _e273: vec2<f32> = v_68;
    let _e275: VoxelScene = u;
    return mix(_e271.x, _e273.y, _e275.volK.w);
}

fn fogDensity(p_40: vec3<f32>) -> f32 {
    var p_41: vec3<f32>;

    p_41 = p_40;
    let _e259: VoxelScene = u;
    if (_e259.fogK.x <= 0f) {
        {
            return 0f;
        }
    }
    let _e265: VoxelScene = u;
    let _e268: f32 = fogPixelNoise;
    let _e270: vec3<f32> = p_41;
    let _e272: f32 = fogShape(_e270.y);
    return ((_e265.fogK.x * _e268) * _e272);
}

fn airDensity(p_42: vec3<f32>) -> f32 {
    var p_43: vec3<f32>;

    p_43 = p_42;
    let _e259: VoxelScene = u;
    let _e262: vec3<f32> = p_43;
    let _e267: VoxelScene = u;
    let _e273: vec3<f32> = p_43;
    let _e274: f32 = fogDensity(_e273);
    return ((_e259.volK.x * exp((-(max(_e262.y, 0f)) / _e267.volK.z))) + _e274);
}

fn inScatter(zEnd_9: i32) -> vec4<f32> {
    var zEnd_10: i32;
    var k_21: f32;
    var n_8: i32;
    var dz_2: f32;
    var ds_1: f32;
    var xw_6: f32;
    var y0_12: f32;
    var acc_2: vec3<f32> = vec3(0f);
    var through: f32 = 1f;
    var i_16: i32 = 0i;
    var zs: f32;
    var p_44: vec3<f32>;
    var fog: f32;
    var d_10: f32;
    var sun_7: vec3<f32>;
    var light: vec3<f32>;
    var local_143: vec3<f32>;
    var glow_5: vec3<f32>;
    var here_1: vec3<f32>;

    zEnd_10 = zEnd_9;
    let _e260: i32 = zEnd_10;
    if (_e260 <= 0i) {
        {
            return vec4<f32>(0f, 0f, 0f, 1f);
        }
    }
    let _e268: i32 = RISE;
    let _e270: i32 = S;
    k_21 = (f32(_e268) / f32(_e270));
    let _e274: i32 = zEnd_10;
    n_8 = min(VOL_SAMPLES, _e274);
    let _e277: i32 = zEnd_10;
    let _e279: i32 = n_8;
    dz_2 = (f32(_e277) / f32(_e279));
    let _e283: f32 = dz_2;
    let _e285: f32 = k_21;
    let _e286: f32 = k_21;
    ds_1 = (_e283 * sqrt((1f + (_e285 * _e286))));
    let _e292: i32 = PX;
    let _e296: i32 = S;
    xw_6 = ((f32(_e292) + 0.5f) / f32(_e296));
    let _e300: i32 = BASE;
    let _e301: i32 = PY;
    let _e308: i32 = S;
    y0_12 = ((f32(((_e300 - _e301) - 1i)) + 0.5f) / f32(_e308));
    let _e312: VoxelScene = u;
    if (_e312.fogK.x > 0f) {
        {
            let _e317: f32 = xw_6;
            let _e318: f32 = y0_12;
            let _e319: f32 = k_21;
            let _e320: i32 = zEnd_10;
            let _e322: f32 = fogNoiseFor(_e317, _e318, _e319, f32(_e320));
            fogPixelNoise = _e322;
        }
    }
    loop {
        let _e330: i32 = i_16;
        let _e331: i32 = n_8;
        if !((_e330 < _e331)) {
            break;
        }
        {
            let _e337: i32 = i_16;
            let _e341: f32 = dz_2;
            zs = ((f32(_e337) + 0.5f) * _e341);
            let _e344: f32 = xw_6;
            let _e345: f32 = y0_12;
            let _e346: f32 = zs;
            let _e347: f32 = k_21;
            let _e350: f32 = zs;
            p_44 = vec3<f32>(_e344, (_e345 - (_e346 * _e347)), _e350);
            let _e353: vec3<f32> = p_44;
            let _e354: f32 = fogDensity(_e353);
            fog = _e354;
            let _e356: vec3<f32> = p_44;
            let _e357: f32 = airDensity(_e356);
            let _e358: f32 = fog;
            let _e361: f32 = zs;
            let _e362: f32 = hazeAt(_e361);
            d_10 = ((_e357 - _e358) * (1f - _e362));
            let _e366: VoxelScene = u;
            let _e369: VoxelScene = u;
            let _e372: vec3<f32> = p_44;
            let _e373: f32 = sunInAir(_e372);
            sun_7 = (_e366.sunLean.xyz * (_e369.dayL.y * _e373));
            let _e377: vec3<f32> = sun_7;
            let _e379: VoxelScene = u;
            light = (_e377 + (0.5f * _e379.flashC.xyz));
            let _e385: VoxelScene = u;
            let _e390: f32 = fog;
            if (GLOW && ((_e385.volK.y > 0f) || (_e390 > 0f))) {
                let _e395: vec3<f32> = p_44;
                let _e396: vec3<f32> = glowAt(_e395);
                local_143 = _e396;
            } else {
                local_143 = vec3(0f);
            }
            let _e400: vec3<f32> = local_143;
            glow_5 = _e400;
            let _e402: vec3<f32> = light;
            let _e403: VoxelScene = u;
            let _e406: vec3<f32> = glow_5;
            light = (_e402 + (_e403.volK.y * _e406));
            let _e409: f32 = d_10;
            let _e410: vec3<f32> = light;
            here_1 = (_e409 * _e410);
            let _e413: f32 = fog;
            if (_e413 > 0f) {
                {
                    let _e416: vec3<f32> = here_1;
                    let _e417: f32 = fog;
                    let _e418: VoxelScene = u;
                    let _e421: VoxelScene = u;
                    let _e424: vec3<f32> = sun_7;
                    let _e427: VoxelScene = u;
                    let _e430: vec3<f32> = glow_5;
                    here_1 = (_e416 + (_e417 * ((_e418.fogC.xyz + (_e421.rainC.w * _e424)) + (_e427.fogC.w * _e430))));
                }
            }
            let _e435: vec3<f32> = acc_2;
            let _e436: f32 = through;
            let _e437: vec3<f32> = here_1;
            acc_2 = (_e435 + (_e436 * _e437));
            let _e440: f32 = through;
            let _e441: f32 = fog;
            let _e443: f32 = ds_1;
            through = (_e440 * exp((-(_e441) * _e443)));
        }
        continuing {
            let _e334: i32 = i_16;
            i_16 = (_e334 + 1i);
        }
    }
    let _e447: vec3<f32> = acc_2;
    let _e448: f32 = ds_1;
    let _e449: vec3<f32> = (_e447 * _e448);
    let _e450: f32 = through;
    return vec4<f32>(_e449.x, _e449.y, _e449.z, _e450);
}

fn walkSkipsPlant(v_69: vec4<u32>, trans_4: vec3<f32>) -> bool {
    var v_70: vec4<u32>;
    var trans_5: vec3<f32>;

    v_70 = v_69;
    trans_5 = trans_4;
    let _e262: vec4<u32> = v_70;
    let _e263: bool = plantHidden(_e262);
    let _e264: vec3<f32> = trans_5;
    let _e271: i32 = wEntry;
    return ((_e263 && all((_e264 == vec3(1f)))) && (!(LIT) || (_e271 < 0i)));
}

fn worldCol(sx_1: i32) -> i32 {
    var sx_2: i32;
    var rw: i32;
    var v_71: i32;
    var local_144: i32;

    sx_2 = sx_1;
    let _e260: i32 = W;
    let _e261: i32 = S;
    rw = (_e260 * _e261);
    let _e264: i32 = sx_2;
    let _e265: VoxelScene = u;
    let _e269: i32 = rw;
    v_71 = ((_e264 + _e265.view.x) % _e269);
    let _e272: i32 = v_71;
    if (_e272 < 0i) {
        let _e275: i32 = v_71;
        let _e276: i32 = rw;
        local_144 = (_e275 + _e276);
    } else {
        let _e278: i32 = v_71;
        local_144 = _e278;
    }
    let _e280: i32 = local_144;
    return _e280;
}

fn highlighted(x_97: i32, y_86: i32, z_96: i32) -> bool {
    var x_98: i32;
    var y_87: i32;
    var z_97: i32;
    var dxh: i32;
    var key_1: u32;
    var i_17: i32 = 0i;

    x_98 = x_97;
    y_87 = y_86;
    z_97 = z_96;
    let _e264: VoxelScene = u;
    if (_e264.view.y == 0i) {
        {
            return false;
        }
    }
    let _e270: i32 = x_98;
    let _e271: VoxelScene = u;
    let _e275: i32 = W;
    let _e276: i32 = wrapMod((_e270 - _e271.hiMin.x), _e275);
    dxh = _e276;
    let _e278: i32 = dxh;
    let _e279: VoxelScene = u;
    let _e283: i32 = y_87;
    let _e284: VoxelScene = u;
    let _e289: i32 = y_87;
    let _e290: VoxelScene = u;
    let _e295: i32 = z_97;
    let _e296: VoxelScene = u;
    let _e301: i32 = z_97;
    let _e302: VoxelScene = u;
    if (((((_e278 > _e279.hiMax.x) || (_e283 < _e284.hiMin.y)) || (_e289 > _e290.hiMax.y)) || (_e295 < _e296.hiMin.z)) || (_e301 > _e302.hiMax.z)) {
        {
            return false;
        }
    }
    let _e308: VoxelScene = u;
    if (_e308.view.y < 0i) {
        {
            return true;
        }
    }
    let _e314: i32 = x_98;
    let _e315: i32 = wrapX(_e314);
    let _e317: i32 = y_87;
    let _e323: i32 = z_97;
    key_1 = ((u32(_e315) | (u32(_e317) << 12u)) | (u32(_e323) << 22u));
    loop {
        let _e332: i32 = i_17;
        let _e333: VoxelScene = u;
        if !((_e332 < _e333.view.y)) {
            break;
        }
        {
            let _e341: i32 = i_17;
            let _e344: i32 = i_17;
            let _e351: u32 = u.hiCells[(_e344 >> 2u)][(_e341 & 3i)];
            let _e352: u32 = key_1;
            if (_e351 == _e352) {
                {
                    return true;
                }
            }
        }
        continuing {
            let _e338: i32 = i_17;
            i_17 = (_e338 + 1i);
        }
    }
    return false;
}

fn hiTint(c_73: vec3<f32>) -> vec3<f32> {
    var c_74: vec3<f32>;

    c_74 = c_73;
    let _e260: vec3<f32> = c_74;
    let _e263: VoxelScene = u;
    return mix(_e260, vec3(1f), vec3(_e263.hiK.x));
}

fn plantVoxel(c_75: vec4<u32>, v_72: ptr<function, vec4<u32>>) -> bool {
    var c_76: vec4<u32>;
    var p_45: vec3<i32>;

    c_76 = c_75;
    let _e261: vec4<u32> = c_76;
    let _e262: bool = plantDying(_e261);
    if _e262 {
        {
            let _e263: vec4<u32> = c_76;
            let _e264: vec4<u32> = plantCarried(_e263);
            (*v_72) = _e264;
            return true;
        }
    }
    let _e266: vec4<u32> = c_76;
    let _e267: vec3<i32> = plantCellAt(_e266);
    p_45 = _e267;
    let _e269: vec3<i32> = p_45;
    let _e271: vec3<i32> = p_45;
    let _e273: vec3<i32> = p_45;
    let _e275: vec4<u32> = at(_e269.x, _e271.y, _e273.z);
    (*v_72) = _e275;
    let _e276: vec4<u32> = (*v_72);
    let _e277: bool = plantHidden(_e276);
    return _e277;
}

fn plantFaceShows(x_99: i32, y_88: i32, z_98: i32, v_73: vec4<u32>, cap_5: bool, row_19: i32, dx_46: i32) -> bool {
    var x_100: i32;
    var y_89: i32;
    var z_99: i32;
    var v_74: vec4<u32>;
    var cap_6: bool;
    var row_20: i32;
    var dx_47: i32;
    var local_145: i32;
    var q_20: u32;
    var pigment_3: vec4<f32>;
    var tone_2: i32;
    var base_16: vec3<f32>;

    x_100 = x_99;
    y_89 = y_88;
    z_99 = z_98;
    v_74 = v_73;
    cap_6 = cap_5;
    row_20 = row_19;
    dx_47 = dx_46;
    let _e272: vec4<u32> = v_74;
    let _e273: i32 = dx_47;
    let _e274: bool = cap_6;
    if _e274 {
        let _e275: i32 = S;
        let _e276: i32 = row_20;
        local_145 = (_e275 + _e276);
    } else {
        let _e278: i32 = row_20;
        local_145 = _e278;
    }
    let _e280: i32 = local_145;
    let _e281: u32 = cordGlyphTexel(_e272, _e273, _e280);
    q_20 = _e281;
    let _e283: u32 = q_20;
    let _e287: vec4<u32> = v_74;
    let _e292: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e283 & 3u)), i32(_e287.w)), 0i);
    pigment_3 = _e292;
    let _e294: u32 = q_20;
    tone_2 = i32((_e294 >> 2u));
    let _e300: bool = cap_6;
    let _e302: i32 = tone_2;
    if (!(_e300) && (_e302 == 63i)) {
        {
            return false;
        }
    }
    let _e307: i32 = tone_2;
    let _e309: vec4<f32> = pigment_3;
    let _e311: bool = emitsWhole(_e309.w);
    let _e314: vec4<u32> = v_74;
    let _e317: vec3<f32> = styleEmit(i32(_e314.w));
    if ((LIT && ((_e307 == TONE_EMIT) || _e311)) && any((_e317 > vec3(0f)))) {
        {
            return true;
        }
    }
    let _e324: vec4<f32> = pigment_3;
    base_16 = _e324.xyz;
    let _e327: i32 = x_100;
    let _e328: i32 = y_89;
    let _e329: i32 = z_99;
    let _e330: vec4<u32> = v_74;
    let _e331: bool = cap_6;
    let _e332: i32 = dx_47;
    let _e333: i32 = row_20;
    let _e336: bool = plantTexel(_e327, _e328, _e329, _e330, _e331, _e332, _e333, (&base_16));
    return _e336;
}

fn plantCordShows(c_77: vec4<u32>, p_46: vec3<i32>, uv: vec2<f32>) -> bool {
    var c_78: vec4<u32>;
    var p_47: vec3<i32>;
    var uv_1: vec2<f32>;
    var wc: f32;
    var wf: vec2<f32>;
    var sh_4: vec2<f32>;
    var e_11: vec2<f32>;

    c_78 = c_77;
    p_47 = p_46;
    uv_1 = uv;
    let _e266: vec4<u32> = c_78;
    let _e267: f32 = plantWeight(_e266);
    wc = _e267;
    let _e269: f32 = wc;
    wf = vec2(_e269);
    let _e272: vec4<u32> = c_78;
    let _e273: bool = plantDying(_e272);
    let _e275: vec4<u32> = c_78;
    if (!(_e273) && ((_e275.w >> 31u) != 0u)) {
        {
            let _e283: vec4<u32> = c_78;
            let _e291: vec4<u32> = c_78;
            wf = (vec2<f32>(f32(((_e283.w >> 12u) & 4095u)), f32((_e291.w & 4095u))) / vec2(4095f));
        }
    }
    let _e300: vec4<u32> = c_78;
    let _e301: u32 = plantOf(_e300);
    let _e303: f32 = global_2.tips[_e301];
    let _e304: vec2<f32> = wf;
    let _e305: f32 = wc;
    let _e309: i32 = S;
    sh_4 = ((_e303 * (_e304 - vec2(_e305))) * f32(_e309));
    let _e313: bool = cordWave();
    if _e313 {
        {
            let _e314: vec3<i32> = p_47;
            let _e315: vec2<f32> = uv_1;
            let _e317: vec2<f32> = uv_1;
            let _e319: i32 = S;
            let _e323: f32 = cordAmp(false);
            let _e324: vec2<f32> = sh_4;
            let _e325: bool = cordWaveCovers(_e314, _e315.y, (_e317.x * f32(_e319)), _e323, _e324);
            return _e325;
        }
    }
    let _e326: vec3<i32> = p_47;
    let _e328: vec3<i32> = p_47;
    let _e330: vec3<i32> = p_47;
    let _e332: vec2<f32> = cordEnds(_e326.x, _e328.y, _e330.z);
    let _e333: vec2<f32> = sh_4;
    e_11 = (_e332 + _e333);
    let _e336: vec2<f32> = e_11;
    let _e338: vec2<f32> = e_11;
    let _e340: vec2<f32> = uv_1;
    let _e342: vec2<f32> = uv_1;
    let _e344: i32 = S;
    let _e347: bool = cordCovers(_e336.x, _e338.y, _e340.y, (_e342.x * f32(_e344)));
    return _e347;
}

fn main_1() {
    var inst: i32;
    var cap_7: bool;
    var c_79: vec4<u32>;
    var v_75: vec4<u32>;
    var p_48: vec3<i32>;
    var local_146: i32;
    var rows_2: i32;
    var uv_2: vec2<f32>;
    var aa: f32;
    var centre_1: vec2<f32>;
    var dx_48: i32;
    var row_21: i32;
    var local_147: u32;

    let _e260: VoxelScene = u;
    S = _e260.geom.x;
    let _e263: VoxelScene = u;
    RISE = _e263.geom.y;
    let _e266: VoxelScene = u;
    BASE = _e266.geom.z;
    let _e269: VoxelScene = u;
    W = _e269.extent.x;
    let _e272: VoxelScene = u;
    H = _e272.extent.y;
    let _e275: VoxelScene = u;
    D = _e275.extent.z;
    let _e278: u32 = vId_1;
    inst = i32((_e278 >> 1u));
    let _e284: u32 = vId_1;
    cap_7 = ((_e284 & 1u) != 0u);
    let _e290: i32 = inst;
    let _e292: vec4<u32> = global_1.cells[_e290];
    c_79 = _e292;
    let _e295: vec4<u32> = c_79;
    let _e298: bool = plantVoxel(_e295, (&v_75));
    if !(_e298) {
        {
            discard;
        }
    }
    let _e300: vec4<u32> = c_79;
    let _e301: vec3<i32> = plantCellAt(_e300);
    p_48 = _e301;
    let _e303: bool = cap_7;
    if _e303 {
        let _e304: i32 = RISE;
        local_146 = _e304;
    } else {
        let _e305: i32 = S;
        local_146 = _e305;
    }
    let _e307: i32 = local_146;
    rows_2 = _e307;
    let _e309: vec2<f32> = vUV_1;
    uv_2 = _e309;
    let _e311: vec4<u32> = v_75;
    let _e312: bool = cordCell(_e311);
    if _e312 {
        {
            let _e313: bool = cap_7;
            if _e313 {
                {
                    discard;
                }
            }
            let _e314: PlantPush = pc;
            aa = f32(_e314.k.y);
            let _e320: vec4<f32> = gl_FragCoord_1;
            let _e322: f32 = aa;
            let _e329: f32 = aa;
            centre_1 = ((floor((_e320.xy / vec2(_e322))) + vec2(0.5f)) * _e329);
            let _e332: vec2<f32> = uv_2;
            let _e333: vec2<f32> = centre_1;
            let _e334: vec4<f32> = gl_FragCoord_1;
            let _e337: f32 = aa;
            let _e338: vec4<u32> = c_79;
            let _e339: f32 = plantScale(_e338);
            let _e343: i32 = S;
            uv_2 = (_e332 + ((_e333 - _e334.xy) / vec2(((_e337 * max(_e339, 0.001f)) * f32(_e343)))));
            let _e349: vec4<u32> = c_79;
            let _e350: vec3<i32> = p_48;
            let _e351: vec2<f32> = uv_2;
            let _e352: bool = plantCordShows(_e349, _e350, _e351);
            if !(_e352) {
                {
                    discard;
                }
            }
        }
    } else {
        let _e354: vec2<f32> = uv_2;
        let _e358: vec2<f32> = uv_2;
        if ((_e354.x < 0f) || (_e358.x >= 1f)) {
            {
                discard;
            }
        }
    }
    let _e363: vec2<f32> = uv_2;
    let _e365: i32 = S;
    let _e370: i32 = S;
    dx_48 = clamp(i32((_e363.x * f32(_e365))), 0i, (_e370 - 1i));
    let _e375: vec2<f32> = uv_2;
    let _e377: i32 = rows_2;
    let _e382: i32 = rows_2;
    row_21 = clamp(i32((_e375.y * f32(_e377))), 0i, (_e382 - 1i));
    let _e387: vec3<i32> = p_48;
    let _e389: vec3<i32> = p_48;
    let _e391: vec3<i32> = p_48;
    let _e393: vec4<u32> = v_75;
    let _e394: bool = cap_7;
    let _e395: i32 = row_21;
    let _e396: i32 = dx_48;
    let _e397: bool = plantFaceShows(_e387.x, _e389.y, _e391.z, _e393, _e394, _e395, _e396);
    if !(_e397) {
        {
            discard;
        }
    }
    let _e399: i32 = inst;
    let _e406: bool = cap_7;
    if _e406 {
        local_147 = 4096u;
    } else {
        local_147 = 0u;
    }
    let _e410: u32 = local_147;
    let _e412: i32 = dx_48;
    let _e418: i32 = row_21;
    outId = ((((u32((_e399 + 1i)) << 13u) | _e410) | (u32(_e412) << 6u)) | u32(_e418));
    return;
}

@fragment 
fn main(@location(0) @interpolate(flat) vId: u32, @location(1) vUV: vec2<f32>, @builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    vId_1 = vId;
    vUV_1 = vUV;
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e392: u32 = outId;
    return FragmentOutput(_e392);
}
