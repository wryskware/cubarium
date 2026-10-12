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

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
    @location(1) emitOut: vec4<f32>,
    @location(2) depthOut: f32,
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
var<private> outColour: vec4<f32>;
var<private> emitOut: vec4<f32>;
var<private> depthOut: f32;
var<private> wEntry: i32;
var<private> wScatter: vec3<f32>;
var<private> wHaze: f32;
var<private> wSurf: f32;
var<private> PX: i32;
var<private> PY: i32;
@group(0) @binding(15) 
var wgpu_weatherPlantZ: texture_2d<f32>;
@group(0) @binding(16) 
var wgpu_weatherFigureZ: texture_depth_2d;
@group(0) @binding(17) 
var wgpu_weatherPlantSurface: texture_2d<f32>;
@group(0) @binding(18) 
var wgpu_weatherFigureSurface: texture_2d<f32>;
@group(0) @binding(10) 
var wgpu_sunVisTex: texture_3d<f32>;
var<private> fogPixelNoise: f32 = 1f;
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
    let _e176: i32 = slot_1;
    return ((_e169 >= 0i) && ((u.tex.x & (1i << u32(_e176))) != 0i));
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
    let _e178: vec4<u32> = v_12;
    let _e185: vec4<u32> = v_12;
    return (((u.tex.y != 0i) && ((_e178.x & 31u) == 0u)) && ((_e185.x >> 5u) != 0u));
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
    let _e178: vec4<u32> = v_18;
    let _e179: i32 = partOf(_e178);
    let _e183: vec4<u32> = v_18;
    let _e188: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e183.w)), 0i);
    return (((u.tex.z > 0i) && (_e179 == CROWN)) && (i32((_e188.w + 0.5f)) == ROLE_DRAPE));
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
    w = u.tex.z;
    let _e181: i32 = S;
    let _e182: i32 = w;
    free = max((_e181 - _e182), 0i);
    let _e187: i32 = free;
    span = i32((((f32(_e187) * f32(((u.tex.w >> 8u) & 255i))) / 100f) + 0.5f));
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
    if ((u.tex.w & 1i) == 1i) {
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
    w_1 = f32(u.tex.z);
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
    return (f32(((u.tex.w >> 16u) & 255i)) / 4f);
}

fn cordWave() -> bool {
    return (((u.tex.w >> 16u) & 255i) != 0i);
}

fn cordAmp(walk: bool) -> f32 {
    var walk_1: bool;
    var a_7: f32;
    var local_6: f32;

    walk_1 = walk;
    a_7 = (f32(((u.tex.w >> 24u) & 255i)) / 32f);
    let _e185: bool = walk_1;
    if _e185 {
        let _e186: f32 = a_7;
        let _e187: i32 = S;
        let _e194: i32 = S;
        local_6 = min(_e186, max((f32((_e187 - u.tex.z)) / (2f * f32(_e194))), 0f));
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
    if ((_e183 == 0i) || ((u.tex.w & 2i) == 0i)) {
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
    return max(((u.tex.w >> 4u) & 15i), 1i);
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
    w_2 = f32(u.tex.z);
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
            return u.skyC.xyz;
        }
    }
    let _e179: i32 = m_3;
    if (_e179 == 1i) {
        {
            return u.bedrockC.xyz;
        }
    }
    let _e185: i32 = m_3;
    if (_e185 == 2i) {
        {
            return u.rockC.xyz;
        }
    }
    return u.soilC.xyz;
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
    let _e183: f32 = zf_1;
    let _e184: i32 = D;
    return (clamp(u.knobs.x, 0f, 1f) * clamp((_e183 / f32((_e184 - 1i))), 0f, 1f));
}

fn hazed(c_6: vec3<f32>, t_10: f32) -> vec3<f32> {
    var c_7: vec3<f32>;
    var t_11: f32;

    c_7 = c_6;
    t_11 = t_10;
    let _e175: vec3<f32> = c_7;
    let _e179: f32 = t_11;
    return mix(_e175, u.hazeC.xyz, vec3(clamp(_e179, 0f, 1f)));
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
    t_12 = (1f - exp((-(f32((_e178 - 1i))) / u.roofK.y)));
    let _e198: f32 = t_12;
    return (u.roofK.x + ((1f - u.roofK.x) * _e198));
}

fn roofGap(x_32: i32, y_26: i32, z_32: i32) -> i32 {
    var x_33: i32;
    var y_27: i32;
    var z_33: i32;
    var yy: i32;

    x_33 = x_32;
    y_27 = y_26;
    z_33 = z_32;
    if (u.geom.w != 0i) {
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
    cap = (u32(u.view.w) & 1048575u);
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
            let _e235: u32 = h_4;
            let _e239: vec4<u32> = global.emitCells[(u.view.z + i32(_e235))];
            c_8 = _e239;
            let _e241: vec4<u32> = c_8;
            let _e243: u32 = key;
            if (_e241.x == _e243) {
                {
                    let _e248: u32 = h_4;
                    return (u.view.z + i32(_e248));
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
    if ((_e175 < 0i) || ((u32(u.view.w) & EMIT_VINES) == 0u)) {
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
    if (!(PLANT_LIGHT) || ((u32(u.view.w) & EMIT_PLANTS) == 0u)) {
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
    let _e190: i32 = dx_11;
    let _e193: i32 = dx_11;
    let _e194: i32 = S;
    if ((!(PLANT_LIGHT) || ((u32(u.view.w) & EMIT_PLANTS) == 0u)) || ((_e190 != 0i) && (_e193 != (_e194 - 1i)))) {
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
    return (-((_e179 * max((_e180 - (clamp(_e181, 0f, 1f) * u.hazeC.xyz)), vec3(0f)))) - vec3(1f));
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
    lit = mix((_e178 * u.plantA.x), u.lightC.xyz, vec3(u.plantA.y));
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
    let _e199: f32 = wet;
    return mix(_e195, u.waterDeepC.xyz, vec3((_e199 * u.shadeB.w)));
}

fn blockLit(body: vec3<f32>, shade_2: f32) -> vec3<f32> {
    var body_1: vec3<f32>;
    var shade_3: f32;
    var lit_1: vec3<f32>;
    var local_13: vec3<f32>;

    body_1 = body;
    shade_3 = shade_2;
    let _e178: vec3<f32> = body_1;
    lit_1 = mix((_e178 * u.shadeA.x), u.lightC.xyz, vec3(u.shadeA.y));
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
                local_16 = (1f - u.lightK.w);
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
    let _e347: f32 = k_5;
    return (1f - (u.lightK.w * _e347));
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
    let _e289: f32 = k_6;
    return (1f - (u.lightK.w * _e289));
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
    smoothLight = (u.lightK.z < 1.5f);
    n_1 = max((u.lightK.z - 1f), 1f);
    hue = u.ambientC.xyz;
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
        local_28 = (clamp(_e245, 0f, 1f) + (_e249 * u.dayK.y));
    } else {
        let _e255: f32 = a_10;
        let _e259: f32 = n_1;
        let _e264: f32 = sun_1;
        let _e270: f32 = n_1;
        local_28 = ((floor(((clamp(_e255, 0f, 1f) * _e259) + 0.5f)) + (_e264 * u.dayK.x)) / _e270);
    }
    let _e273: f32 = local_28;
    t_15 = _e273;
    let _e275: vec3<f32> = hue;
    let _e287: f32 = t_15;
    return (_e275 * (u.lightK.x * (u.lightK.y + ((1f - u.lightK.y) * _e287))));
}

fn ladderRef(sunOn: f32) -> vec3<f32> {
    var sunOn_1: f32;
    var smoothLight_1: bool;
    var n_2: f32;
    var local_29: f32;
    var t_16: f32;

    sunOn_1 = sunOn;
    smoothLight_1 = (u.lightK.z < 1.5f);
    n_2 = max((u.lightK.z - 1f), 1f);
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
    let _e226: f32 = t_16;
    return ((u.ambientC.xyz / vec3(u.dayK.z)) * (u.lightK.x * (u.lightK.y + ((1f - u.lightK.y) * _e226))));
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
    if (u.figSun.w < 0.5f) {
        {
            return false;
        }
    }
    let _e184: vec3<f32> = p_16;
    let _e186: vec3<f32> = p_16;
    fx = (_e184.x - (_e186.y * u.figSun.x));
    let _e194: vec3<f32> = p_16;
    let _e196: vec3<f32> = p_16;
    fz = ((_e194.z - (_e196.y * u.figSun.y)) - u.figSun.z);
    let _e208: f32 = fx;
    let _e209: i32 = W;
    let _e211: f32 = fx;
    let _e212: i32 = W;
    let _e222: f32 = fz;
    t_17 = vec2<f32>(((_e208 - (f32(_e209) * floor((_e211 / f32(_e212))))) * u.figMap.x), (_e222 * u.figMap.y));
    let _e229: vec2<f32> = t_17;
    let _e233: vec2<f32> = t_17;
    if ((_e229.y < 0f) || (_e233.y >= u.figMap.w)) {
        {
            return false;
        }
    }
    let _e241: vec2<f32> = t_17;
    let _e251: f32 = textureLoad(wgpu_figSunTex, vec2<i32>(min(_e241, (u.figMap.zw - vec2(1f)))), 0i);
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
    L = u.sunK.xyz;
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
    let _e200: f32 = sun_3;
    let _e202: f32 = ndl_1;
    return mix(_e193, u.sunLean.xyz, vec3(clamp(((u.sunK.w * _e200) * _e202), 0f, 1f)));
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
    ndl_2 = max(-(u.sunK.z), 0f);
    let _e199: i32 = x_55;
    let _e201: i32 = dx_21;
    let _e205: i32 = S;
    let _e209: i32 = y_47;
    let _e213: i32 = dy_15;
    let _e217: i32 = S;
    let _e221: i32 = z_55;
    p_19 = vec3<f32>((f32(_e199) + ((f32(_e201) + 0.5f) / f32(_e205))), (f32((_e209 + 1i)) - ((f32(_e213) + 0.5f) / f32(_e217))), f32(_e221));
    let _e225: f32 = ndl_2;
    if ((_e225 > 0f) && (u.sunK.y > 0f)) {
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
    ndl_3 = max(u.sunK.y, 0f);
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

fn faceGrain(x_58: i32, y_50: i32, z_58: i32, dx_24: i32, dy_18: i32) -> f32 {
    var x_59: i32;
    var y_51: i32;
    var z_59: i32;
    var dx_25: i32;
    var dy_19: i32;
    var h_6: u32;

    x_59 = x_58;
    y_51 = y_50;
    z_59 = z_58;
    dx_25 = dx_24;
    dy_19 = dy_18;
    let _e190: i32 = x_59;
    let _e193: i32 = y_51;
    let _e197: i32 = z_59;
    let _e201: i32 = dx_25;
    let _e207: i32 = dy_19;
    h_6 = u32(((u32((((_e190 * 73856093i) ^ (_e193 * 19349663i)) ^ (_e197 * 83492791i))) ^ (u32(_e201) * 2654435761u)) ^ u32((_e207 * 38291i))));
    let _e214: u32 = h_6;
    let _e215: u32 = h_6;
    h_6 = ((_e214 ^ (_e215 >> 13u)) * 1274126177u);
    let _e222: u32 = h_6;
    return (f32((i32((_e222 & 15u)) - 7i)) * 0.005f);
}

fn blockFrontLit(x_60: i32, y_52: i32, z_60: i32, v_32: vec4<u32>, r_6: i32, dx_26: i32) -> vec3<f32> {
    var x_61: i32;
    var y_53: i32;
    var z_61: i32;
    var v_33: vec4<u32>;
    var r_7: i32;
    var dx_27: i32;
    var dy_20: i32;
    var openUp: bool;
    var openLeft: bool;
    var openRight: bool;
    var riser: bool;
    var onSide: bool;
    var m_9: i32;
    var local_42: i32;
    var slot_6: i32;
    var body_2: vec3<f32>;
    var own: vec3<f32>;
    var lit_2: vec3<f32>;
    var local_43: f32;
    var t_18: f32;
    var slope: vec3<f32>;
    var hz: f32;
    var local_44: vec3<f32>;
    var haze_2: f32;
    var c_19: vec3<f32>;
    var lit_3: vec3<f32>;
    var local_45: vec3<f32>;
    var local_46: vec3<f32>;

    x_61 = x_60;
    y_53 = y_52;
    z_61 = z_60;
    v_33 = v_32;
    r_7 = r_6;
    dx_27 = dx_26;
    let _e192: i32 = S;
    let _e195: i32 = r_7;
    dy_20 = ((_e192 - 1i) - _e195);
    let _e198: i32 = x_61;
    let _e199: i32 = y_53;
    let _e202: i32 = z_61;
    let _e203: bool = solidAt(_e198, (_e199 + 1i), _e202);
    openUp = !(_e203);
    let _e206: i32 = x_61;
    let _e209: i32 = y_53;
    let _e210: i32 = z_61;
    let _e211: bool = solidAt((_e206 - 1i), _e209, _e210);
    openLeft = !(_e211);
    let _e214: i32 = x_61;
    let _e217: i32 = y_53;
    let _e218: i32 = z_61;
    let _e219: bool = solidAt((_e214 + 1i), _e217, _e218);
    openRight = !(_e219);
    let _e222: bool = openUp;
    let _e223: i32 = z_61;
    let _e227: i32 = x_61;
    let _e228: i32 = y_53;
    let _e229: i32 = z_61;
    let _e232: bool = solidAt(_e227, _e228, (_e229 - 1i));
    let _e235: i32 = x_61;
    let _e236: i32 = y_53;
    let _e239: i32 = z_61;
    let _e242: bool = solidAt(_e235, (_e236 - 1i), (_e239 - 1i));
    riser = (((_e222 && (_e223 > 0i)) && !(_e232)) && _e242);
    let _e245: i32 = dx_27;
    let _e248: bool = openLeft;
    let _e250: i32 = dx_27;
    let _e253: i32 = S;
    let _e255: bool = openRight;
    onSide = (((_e245 == 0i) && _e248) || (((_e250 + 1i) == _e253) && _e255));
    let _e259: vec4<u32> = v_33;
    let _e260: i32 = matOf(_e259);
    m_9 = _e260;
    let _e262: i32 = m_9;
    let _e265: bool = openUp;
    let _e267: bool = texOn(TEX_TURF_SIDE);
    if (((_e262 == 3i) && _e265) && _e267) {
        local_42 = TEX_TURF_SIDE;
    } else {
        let _e270: i32 = m_9;
        local_42 = (2i * (_e270 - 1i));
    }
    let _e275: i32 = local_42;
    slot_6 = _e275;
    let _e277: vec4<u32> = v_33;
    let _e278: i32 = m_9;
    let _e279: i32 = slot_6;
    let _e280: i32 = x_61;
    let _e281: i32 = y_53;
    let _e282: i32 = z_61;
    let _e284: i32 = dx_27;
    let _e285: i32 = dy_20;
    let _e286: vec3<f32> = faceBase(_e278, _e279, _e280, _e281, _e282, 0i, _e284, _e285);
    let _e287: vec3<f32> = blockBody(_e277, _e286);
    body_2 = _e287;
    let _e289: i32 = m_9;
    if ((_e289 != 0i) && (u.roofK.z > 0f)) {
        {
            let _e298: vec3<f32> = body_2;
            let _e300: i32 = x_61;
            let _e301: i32 = y_53;
            let _e302: i32 = z_61;
            let _e303: i32 = dx_27;
            let _e304: i32 = dy_20;
            let _e305: f32 = faceGrain(_e300, _e301, _e302, _e303, _e304);
            body_2 = (_e298 * (1f + (_e305 * (u.roofK.z / 0.04f))));
        }
    }
    let _e314: vec3<f32> = body_2;
    let _e315: i32 = x_61;
    let _e316: i32 = y_53;
    let _e317: i32 = z_61;
    let _e318: i32 = dx_27;
    let _e319: i32 = dy_20;
    let _e320: i32 = x_61;
    let _e321: i32 = z_61;
    let _e324: i32 = y_53;
    let _e325: f32 = canopyAt(_e320, (_e321 - 1i), _e324);
    let _e326: vec3<f32> = shadeFront(_e314, _e315, _e316, _e317, _e318, _e319, _e325);
    own = _e326;
    let _e328: bool = riser;
    if _e328 {
        {
            let _e329: vec3<f32> = body_2;
            let _e330: i32 = x_61;
            let _e331: i32 = y_53;
            let _e332: i32 = z_61;
            let _e333: i32 = dx_27;
            let _e334: i32 = RISE;
            let _e337: i32 = x_61;
            let _e338: i32 = z_61;
            let _e339: i32 = y_53;
            let _e340: f32 = canopyAt(_e337, _e338, _e339);
            let _e341: vec3<f32> = shadeTop(_e329, _e330, _e331, _e332, _e333, (_e334 - 1i), _e340);
            lit_2 = _e341;
            let _e343: i32 = S;
            if (_e343 > 1i) {
                let _e346: i32 = dy_20;
                let _e348: i32 = S;
                local_43 = (f32(_e346) / f32((_e348 - 1i)));
            } else {
                local_43 = 0f;
            }
            let _e355: f32 = local_43;
            t_18 = _e355;
            let _e357: vec3<f32> = lit_2;
            let _e358: vec3<f32> = own;
            let _e362: f32 = t_18;
            slope = mix(_e357, _e358, vec3((u.shadeB.z * _e362)));
            let _e367: i32 = z_61;
            let _e369: f32 = t_18;
            let _e371: f32 = hazeAt((f32(_e367) - _e369));
            hz = _e371;
            let _e373: bool = onSide;
            if _e373 {
                let _e374: vec3<f32> = slope;
                local_44 = (_e374 * u.shadeB.x);
            } else {
                let _e379: vec3<f32> = slope;
                local_44 = _e379;
            }
            let _e381: vec3<f32> = local_44;
            let _e382: f32 = hz;
            let _e383: vec3<f32> = hazed(_e381, _e382);
            return _e383;
        }
    }
    let _e384: i32 = z_61;
    let _e386: f32 = hazeAt(f32(_e384));
    haze_2 = _e386;
    let _e389: i32 = dy_20;
    let _e392: bool = openUp;
    if ((_e389 == 0i) && _e392) {
        {
            let _e394: vec3<f32> = body_2;
            let _e395: i32 = x_61;
            let _e396: i32 = y_53;
            let _e397: i32 = z_61;
            let _e398: i32 = dx_27;
            let _e399: i32 = RISE;
            let _e402: i32 = x_61;
            let _e403: i32 = z_61;
            let _e404: i32 = y_53;
            let _e405: f32 = canopyAt(_e402, _e403, _e404);
            let _e406: vec3<f32> = shadeTop(_e394, _e395, _e396, _e397, _e398, (_e399 - 1i), _e405);
            lit_3 = _e406;
            let _e408: bool = onSide;
            if _e408 {
                let _e409: vec3<f32> = lit_3;
                local_45 = _e409;
            } else {
                let _e410: vec3<f32> = own;
                let _e411: vec3<f32> = lit_3;
                local_45 = mix(_e410, _e411, vec3(u.shadeA.w));
            }
            let _e418: vec3<f32> = local_45;
            c_19 = _e418;
        }
    } else {
        {
            let _e419: bool = onSide;
            if _e419 {
                let _e420: vec3<f32> = own;
                local_46 = (_e420 * u.shadeB.x);
            } else {
                let _e425: vec3<f32> = own;
                local_46 = _e425;
            }
            let _e427: vec3<f32> = local_46;
            c_19 = _e427;
        }
    }
    let _e428: vec3<f32> = c_19;
    let _e429: f32 = haze_2;
    let _e430: vec3<f32> = hazed(_e428, _e429);
    return _e430;
}

fn blockFront(x_62: i32, y_54: i32, z_62: i32, v_34: vec4<u32>, r_8: i32, dx_28: i32) -> vec3<f32> {
    var x_63: i32;
    var y_55: i32;
    var z_63: i32;
    var v_35: vec4<u32>;
    var r_9: i32;
    var dx_29: i32;
    var dy_21: i32;
    var openUp_1: bool;
    var openLeft_1: bool;
    var openRight_1: bool;
    var riser_1: bool;
    var onSide_1: bool;
    var m_10: i32;
    var local_47: i32;
    var slot_7: i32;
    var body_3: vec3<f32>;
    var lit_4: vec3<f32>;
    var local_48: f32;
    var t_19: f32;
    var slope_1: vec3<f32>;
    var hz_1: f32;
    var local_49: vec3<f32>;
    var haze_3: f32;
    var onCap: bool;
    var c_20: vec3<f32>;
    var local_50: vec3<f32>;
    var local_51: vec3<f32>;

    x_63 = x_62;
    y_55 = y_54;
    z_63 = z_62;
    v_35 = v_34;
    r_9 = r_8;
    dx_29 = dx_28;
    if LIT {
        {
            let _e192: i32 = x_63;
            let _e193: i32 = y_55;
            let _e194: i32 = z_63;
            let _e195: vec4<u32> = v_35;
            let _e196: i32 = r_9;
            let _e197: i32 = dx_29;
            let _e198: vec3<f32> = blockFrontLit(_e192, _e193, _e194, _e195, _e196, _e197);
            return _e198;
        }
    }
    let _e199: i32 = S;
    let _e202: i32 = r_9;
    dy_21 = ((_e199 - 1i) - _e202);
    let _e205: i32 = x_63;
    let _e206: i32 = y_55;
    let _e209: i32 = z_63;
    let _e210: bool = solidAt(_e205, (_e206 + 1i), _e209);
    openUp_1 = !(_e210);
    let _e213: i32 = x_63;
    let _e216: i32 = y_55;
    let _e217: i32 = z_63;
    let _e218: bool = solidAt((_e213 - 1i), _e216, _e217);
    openLeft_1 = !(_e218);
    let _e221: i32 = x_63;
    let _e224: i32 = y_55;
    let _e225: i32 = z_63;
    let _e226: bool = solidAt((_e221 + 1i), _e224, _e225);
    openRight_1 = !(_e226);
    let _e229: bool = openUp_1;
    let _e230: i32 = z_63;
    let _e234: i32 = x_63;
    let _e235: i32 = y_55;
    let _e236: i32 = z_63;
    let _e239: bool = solidAt(_e234, _e235, (_e236 - 1i));
    let _e242: i32 = x_63;
    let _e243: i32 = y_55;
    let _e246: i32 = z_63;
    let _e249: bool = solidAt(_e242, (_e243 - 1i), (_e246 - 1i));
    riser_1 = (((_e229 && (_e230 > 0i)) && !(_e239)) && _e249);
    let _e252: i32 = dx_29;
    let _e255: bool = openLeft_1;
    let _e257: i32 = dx_29;
    let _e260: i32 = S;
    let _e262: bool = openRight_1;
    onSide_1 = (((_e252 == 0i) && _e255) || (((_e257 + 1i) == _e260) && _e262));
    let _e266: vec4<u32> = v_35;
    let _e267: i32 = matOf(_e266);
    m_10 = _e267;
    let _e269: i32 = m_10;
    let _e272: bool = openUp_1;
    let _e274: bool = texOn(TEX_TURF_SIDE);
    if (((_e269 == 3i) && _e272) && _e274) {
        local_47 = TEX_TURF_SIDE;
    } else {
        let _e277: i32 = m_10;
        local_47 = (2i * (_e277 - 1i));
    }
    let _e282: i32 = local_47;
    slot_7 = _e282;
    let _e284: vec4<u32> = v_35;
    let _e285: i32 = m_10;
    let _e286: i32 = slot_7;
    let _e287: i32 = x_63;
    let _e288: i32 = y_55;
    let _e289: i32 = z_63;
    let _e291: i32 = dx_29;
    let _e292: i32 = dy_21;
    let _e293: vec3<f32> = faceBase(_e285, _e286, _e287, _e288, _e289, 0i, _e291, _e292);
    let _e294: vec3<f32> = blockBody(_e284, _e293);
    body_3 = _e294;
    let _e296: i32 = m_10;
    if ((_e296 != 0i) && (u.roofK.z > 0f)) {
        {
            let _e305: vec3<f32> = body_3;
            let _e307: i32 = x_63;
            let _e308: i32 = y_55;
            let _e309: i32 = z_63;
            let _e310: i32 = dx_29;
            let _e311: i32 = dy_21;
            let _e312: f32 = faceGrain(_e307, _e308, _e309, _e310, _e311);
            body_3 = (_e305 * (1f + (_e312 * (u.roofK.z / 0.04f))));
        }
    }
    let _e321: vec3<f32> = body_3;
    let _e322: i32 = x_63;
    let _e323: i32 = y_55;
    let _e324: i32 = z_63;
    let _e325: i32 = roofGap(_e322, _e323, _e324);
    let _e326: f32 = roofShade(_e325);
    let _e327: vec3<f32> = blockLit(_e321, _e326);
    lit_4 = _e327;
    let _e329: bool = riser_1;
    if _e329 {
        {
            let _e330: i32 = S;
            if (_e330 > 1i) {
                let _e333: i32 = dy_21;
                let _e335: i32 = S;
                local_48 = (f32(_e333) / f32((_e335 - 1i)));
            } else {
                local_48 = 0f;
            }
            let _e342: f32 = local_48;
            t_19 = _e342;
            let _e344: vec3<f32> = lit_4;
            let _e345: vec3<f32> = body_3;
            let _e349: f32 = t_19;
            slope_1 = mix(_e344, _e345, vec3((u.shadeB.z * _e349)));
            let _e354: i32 = z_63;
            let _e356: f32 = t_19;
            let _e358: f32 = hazeAt((f32(_e354) - _e356));
            hz_1 = _e358;
            let _e360: bool = onSide_1;
            if _e360 {
                let _e361: vec3<f32> = slope_1;
                local_49 = (_e361 * u.shadeB.x);
            } else {
                let _e366: vec3<f32> = slope_1;
                local_49 = _e366;
            }
            let _e368: vec3<f32> = local_49;
            let _e369: f32 = hz_1;
            let _e370: vec3<f32> = hazed(_e368, _e369);
            return _e370;
        }
    }
    let _e371: i32 = z_63;
    let _e373: f32 = hazeAt(f32(_e371));
    haze_3 = _e373;
    let _e375: i32 = dy_21;
    let _e378: bool = openUp_1;
    onCap = ((_e375 == 0i) && _e378);
    let _e382: bool = onCap;
    if _e382 {
        {
            let _e383: bool = onSide_1;
            if _e383 {
                let _e384: vec3<f32> = lit_4;
                local_50 = _e384;
            } else {
                let _e385: vec3<f32> = body_3;
                let _e386: vec3<f32> = lit_4;
                local_50 = mix(_e385, _e386, vec3(u.shadeA.w));
            }
            let _e393: vec3<f32> = local_50;
            c_20 = _e393;
        }
    } else {
        {
            let _e394: bool = onSide_1;
            if _e394 {
                let _e395: vec3<f32> = body_3;
                local_51 = (_e395 * u.shadeB.x);
            } else {
                let _e400: vec3<f32> = body_3;
                local_51 = _e400;
            }
            let _e402: vec3<f32> = local_51;
            c_20 = _e402;
        }
    }
    let _e403: vec3<f32> = c_20;
    let _e404: f32 = haze_3;
    let _e405: vec3<f32> = hazed(_e403, _e404);
    return _e405;
}

fn blockTopLit(x_64: i32, y_56: i32, z_64: i32, v_36: vec4<u32>, r_10: i32, dx_30: i32) -> vec3<f32> {
    var x_65: i32;
    var y_57: i32;
    var z_65: i32;
    var v_37: vec4<u32>;
    var r_11: i32;
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
    var local_52: vec3<f32>;
    var plane: vec3<f32>;
    var onDrop: bool;
    var local_53: vec3<f32>;

    x_65 = x_64;
    y_57 = y_56;
    z_65 = z_64;
    v_37 = v_36;
    r_11 = r_10;
    dx_31 = dx_30;
    let _e192: i32 = RISE;
    let _e195: i32 = r_11;
    dy_22 = ((_e192 - 1i) - _e195);
    let _e198: i32 = x_65;
    let _e201: i32 = y_57;
    let _e202: i32 = z_65;
    let _e203: bool = solidAt((_e198 - 1i), _e201, _e202);
    openLeft_2 = !(_e203);
    let _e206: i32 = x_65;
    let _e209: i32 = y_57;
    let _e210: i32 = z_65;
    let _e211: bool = solidAt((_e206 + 1i), _e209, _e210);
    openRight_2 = !(_e211);
    let _e214: bool = openLeft_2;
    let _e215: i32 = x_65;
    let _e218: i32 = y_57;
    let _e221: i32 = z_65;
    let _e222: bool = solidAt((_e215 - 1i), (_e218 - 1i), _e221);
    dropLeft = (_e214 && !(_e222));
    let _e226: bool = openRight_2;
    let _e227: i32 = x_65;
    let _e230: i32 = y_57;
    let _e233: i32 = z_65;
    let _e234: bool = solidAt((_e227 + 1i), (_e230 - 1i), _e233);
    dropRight = (_e226 && !(_e234));
    let _e238: i32 = z_65;
    let _e241: i32 = D;
    let _e243: i32 = x_65;
    let _e244: i32 = y_57;
    let _e245: i32 = z_65;
    let _e248: bool = solidAt(_e243, _e244, (_e245 + 1i));
    let _e250: i32 = x_65;
    let _e251: i32 = y_57;
    let _e254: i32 = z_65;
    let _e257: bool = solidAt(_e250, (_e251 + 1i), (_e254 + 1i));
    backContinues = ((((_e238 + 1i) < _e241) && _e248) && !(_e257));
    let _e261: vec4<u32> = v_37;
    let _e262: i32 = matOf(_e261);
    m_11 = _e262;
    let _e264: vec4<u32> = v_37;
    let _e265: i32 = m_11;
    let _e267: i32 = m_11;
    let _e273: i32 = x_65;
    let _e274: i32 = y_57;
    let _e275: i32 = z_65;
    let _e277: i32 = dx_31;
    let _e278: i32 = dy_22;
    let _e279: vec3<f32> = faceBase(_e265, ((2i * (_e267 - 1i)) + 1i), _e273, _e274, _e275, 1i, _e277, _e278);
    let _e280: vec3<f32> = blockBody(_e264, _e279);
    body_4 = _e280;
    let _e282: i32 = m_11;
    if ((_e282 != 0i) && (u.roofK.z > 0f)) {
        {
            let _e291: vec3<f32> = body_4;
            let _e293: i32 = x_65;
            let _e294: i32 = y_57;
            let _e295: i32 = z_65;
            let _e296: i32 = dx_31;
            let _e297: i32 = dy_22;
            let _e300: f32 = faceGrain(_e293, _e294, _e295, _e296, (_e297 + 100i));
            body_4 = (_e291 * (1f + (_e300 * (u.roofK.z / 0.04f))));
        }
    }
    let _e309: vec3<f32> = body_4;
    let _e310: i32 = x_65;
    let _e311: i32 = y_57;
    let _e312: i32 = z_65;
    let _e313: i32 = dx_31;
    let _e314: i32 = dy_22;
    let _e315: i32 = x_65;
    let _e316: i32 = z_65;
    let _e317: i32 = y_57;
    let _e318: f32 = canopyAt(_e315, _e316, _e317);
    let _e319: vec3<f32> = shadeTop(_e309, _e310, _e311, _e312, _e313, _e314, _e318);
    own_1 = _e319;
    let _e321: vec3<f32> = own_1;
    dark = (_e321 / vec3(u.shadeA.x));
    let _e328: i32 = z_65;
    let _e330: i32 = r_11;
    let _e332: i32 = RISE;
    let _e336: f32 = hazeAt((f32(_e328) + (f32(_e330) / f32(_e332))));
    hz_2 = _e336;
    let _e338: i32 = dy_22;
    let _e341: i32 = RISE;
    let _e345: bool = backContinues;
    if (((_e338 == 0i) && (_e341 > 2i)) && !(_e345)) {
        let _e348: vec3<f32> = own_1;
        let _e349: vec3<f32> = dark;
        local_52 = mix(_e348, _e349, vec3(u.shadeA.z));
    } else {
        let _e355: vec3<f32> = own_1;
        local_52 = _e355;
    }
    let _e357: vec3<f32> = local_52;
    plane = _e357;
    let _e359: i32 = dx_31;
    let _e362: bool = dropLeft;
    let _e364: i32 = dx_31;
    let _e367: i32 = S;
    let _e369: bool = dropRight;
    onDrop = (((_e359 == 0i) && _e362) || (((_e364 + 1i) == _e367) && _e369));
    let _e373: bool = onDrop;
    if _e373 {
        let _e374: vec3<f32> = plane;
        let _e375: vec3<f32> = dark;
        local_53 = mix(_e374, _e375, vec3(u.shadeB.y));
    } else {
        let _e381: vec3<f32> = plane;
        local_53 = _e381;
    }
    let _e383: vec3<f32> = local_53;
    let _e384: f32 = hz_2;
    let _e385: vec3<f32> = hazed(_e383, _e384);
    return _e385;
}

fn blockTop(x_66: i32, y_58: i32, z_66: i32, v_38: vec4<u32>, r_12: i32, dx_32: i32) -> vec3<f32> {
    var x_67: i32;
    var y_59: i32;
    var z_67: i32;
    var v_39: vec4<u32>;
    var r_13: i32;
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
    var local_54: vec3<f32>;
    var plane_1: vec3<f32>;
    var onDrop_1: bool;
    var local_55: vec3<f32>;

    x_67 = x_66;
    y_59 = y_58;
    z_67 = z_66;
    v_39 = v_38;
    r_13 = r_12;
    dx_33 = dx_32;
    if LIT {
        {
            let _e192: i32 = x_67;
            let _e193: i32 = y_59;
            let _e194: i32 = z_67;
            let _e195: vec4<u32> = v_39;
            let _e196: i32 = r_13;
            let _e197: i32 = dx_33;
            let _e198: vec3<f32> = blockTopLit(_e192, _e193, _e194, _e195, _e196, _e197);
            return _e198;
        }
    }
    let _e199: i32 = RISE;
    let _e202: i32 = r_13;
    dy_23 = ((_e199 - 1i) - _e202);
    let _e205: i32 = x_67;
    let _e208: i32 = y_59;
    let _e209: i32 = z_67;
    let _e210: bool = solidAt((_e205 - 1i), _e208, _e209);
    openLeft_3 = !(_e210);
    let _e213: i32 = x_67;
    let _e216: i32 = y_59;
    let _e217: i32 = z_67;
    let _e218: bool = solidAt((_e213 + 1i), _e216, _e217);
    openRight_3 = !(_e218);
    let _e221: bool = openLeft_3;
    let _e222: i32 = x_67;
    let _e225: i32 = y_59;
    let _e228: i32 = z_67;
    let _e229: bool = solidAt((_e222 - 1i), (_e225 - 1i), _e228);
    dropLeft_1 = (_e221 && !(_e229));
    let _e233: bool = openRight_3;
    let _e234: i32 = x_67;
    let _e237: i32 = y_59;
    let _e240: i32 = z_67;
    let _e241: bool = solidAt((_e234 + 1i), (_e237 - 1i), _e240);
    dropRight_1 = (_e233 && !(_e241));
    let _e245: i32 = z_67;
    let _e248: i32 = D;
    let _e250: i32 = x_67;
    let _e251: i32 = y_59;
    let _e252: i32 = z_67;
    let _e255: bool = solidAt(_e250, _e251, (_e252 + 1i));
    let _e257: i32 = x_67;
    let _e258: i32 = y_59;
    let _e261: i32 = z_67;
    let _e264: bool = solidAt(_e257, (_e258 + 1i), (_e261 + 1i));
    backContinues_1 = ((((_e245 + 1i) < _e248) && _e255) && !(_e264));
    let _e268: vec4<u32> = v_39;
    let _e269: i32 = matOf(_e268);
    m_12 = _e269;
    let _e271: vec4<u32> = v_39;
    let _e272: i32 = m_12;
    let _e274: i32 = m_12;
    let _e280: i32 = x_67;
    let _e281: i32 = y_59;
    let _e282: i32 = z_67;
    let _e284: i32 = dx_33;
    let _e285: i32 = dy_23;
    let _e286: vec3<f32> = faceBase(_e272, ((2i * (_e274 - 1i)) + 1i), _e280, _e281, _e282, 1i, _e284, _e285);
    let _e287: vec3<f32> = blockBody(_e271, _e286);
    body_5 = _e287;
    let _e289: i32 = m_12;
    if ((_e289 != 0i) && (u.roofK.z > 0f)) {
        {
            let _e298: vec3<f32> = body_5;
            let _e300: i32 = x_67;
            let _e301: i32 = y_59;
            let _e302: i32 = z_67;
            let _e303: i32 = dx_33;
            let _e304: i32 = dy_23;
            let _e307: f32 = faceGrain(_e300, _e301, _e302, _e303, (_e304 + 100i));
            body_5 = (_e298 * (1f + (_e307 * (u.roofK.z / 0.04f))));
        }
    }
    let _e316: vec3<f32> = body_5;
    let _e317: i32 = x_67;
    let _e318: i32 = y_59;
    let _e319: i32 = z_67;
    let _e320: i32 = roofGap(_e317, _e318, _e319);
    let _e321: f32 = roofShade(_e320);
    let _e322: vec3<f32> = blockLit(_e316, _e321);
    lit_5 = _e322;
    let _e324: i32 = z_67;
    let _e326: i32 = r_13;
    let _e328: i32 = RISE;
    let _e332: f32 = hazeAt((f32(_e324) + (f32(_e326) / f32(_e328))));
    hz_3 = _e332;
    let _e334: i32 = dy_23;
    let _e337: i32 = RISE;
    let _e341: bool = backContinues_1;
    if (((_e334 == 0i) && (_e337 > 2i)) && !(_e341)) {
        let _e344: vec3<f32> = lit_5;
        let _e345: vec3<f32> = body_5;
        local_54 = mix(_e344, _e345, vec3(u.shadeA.z));
    } else {
        let _e351: vec3<f32> = lit_5;
        local_54 = _e351;
    }
    let _e353: vec3<f32> = local_54;
    plane_1 = _e353;
    let _e355: i32 = dx_33;
    let _e358: bool = dropLeft_1;
    let _e360: i32 = dx_33;
    let _e363: i32 = S;
    let _e365: bool = dropRight_1;
    onDrop_1 = (((_e355 == 0i) && _e358) || (((_e360 + 1i) == _e363) && _e365));
    let _e369: bool = onDrop_1;
    if _e369 {
        let _e370: vec3<f32> = plane_1;
        let _e371: vec3<f32> = body_5;
        local_55 = mix(_e370, _e371, vec3(u.shadeB.y));
    } else {
        let _e377: vec3<f32> = plane_1;
        local_55 = _e377;
    }
    let _e379: vec3<f32> = local_55;
    let _e380: f32 = hz_3;
    let _e381: vec3<f32> = hazed(_e379, _e380);
    return _e381;
}

fn glyphPigment(v_40: vec4<u32>, q_2: u32) -> vec3<f32> {
    var v_41: vec4<u32>;
    var q_3: u32;

    v_41 = v_40;
    q_3 = q_2;
    let _e184: vec4<u32> = v_41;
    let _e187: u32 = q_3;
    let _e191: vec3<f32> = styleAt(i32(_e184.w), i32((_e187 & 3u)));
    return _e191;
}

fn roleSlot(v_42: vec4<u32>, top_1: bool) -> i32 {
    var v_43: vec4<u32>;
    var top_2: bool;
    var role: i32;
    var local_56: i32;

    v_43 = v_42;
    top_2 = top_1;
    let _e185: vec4<u32> = v_43;
    let _e190: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e185.w)), 0i);
    role = i32((_e190.w + 0.5f));
    let _e196: i32 = role;
    let _e198: i32 = role;
    if ((_e196 < ROLE_BARK) || (_e198 > ROLE_DRAPE)) {
        {
            return -1i;
        }
    }
    let _e204: i32 = role;
    let _e208: bool = top_2;
    if _e208 {
        local_56 = 1i;
    } else {
        local_56 = 0i;
    }
    let _e212: i32 = local_56;
    return ((TEX_BARK_SIDE + (2i * (_e204 - ROLE_BARK))) + _e212);
}

fn speciesSlot(v_44: vec4<u32>, top_3: bool) -> i32 {
    var v_45: vec4<u32>;
    var top_4: bool;
    var local_57: i32;

    v_45 = v_44;
    top_4 = top_3;
    let _e184: bool = top_4;
    if _e184 {
        local_57 = 2i;
    } else {
        local_57 = 1i;
    }
    let _e188: i32 = local_57;
    let _e189: vec4<u32> = v_45;
    let _e194: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(_e188, i32(_e189.w)), 0i);
    return (i32((_e194.w + 0.5f)) - 1i);
}

fn cordShows(x_68: i32, y_60: i32, z_68: i32, top_5: bool, dx_34: i32, dy_24: i32) -> bool {
    var x_69: i32;
    var y_61: i32;
    var z_69: i32;
    var top_6: bool;
    var dx_35: i32;
    var dy_25: i32;

    x_69 = x_68;
    y_61 = y_60;
    z_69 = z_68;
    top_6 = top_5;
    dx_35 = dx_34;
    dy_25 = dy_24;
    let _e192: bool = top_6;
    let _e194: i32 = x_69;
    let _e195: i32 = y_61;
    let _e196: i32 = z_69;
    let _e197: i32 = dx_35;
    let _e198: i32 = dy_25;
    let _e199: bool = cordTexel(_e194, _e195, _e196, _e197, _e198);
    return (!(_e192) && _e199);
}

fn plantTexel(x_70: i32, y_62: i32, z_70: i32, v_46: vec4<u32>, top_7: bool, dx_36: i32, dy_26: i32, base_8: ptr<function, vec3<f32>>) -> bool {
    var x_71: i32;
    var y_63: i32;
    var z_71: i32;
    var v_47: vec4<u32>;
    var top_8: bool;
    var dx_37: i32;
    var dy_27: i32;
    var own_2: i32;
    var local_58: i32;
    var t_20: vec4<f32>;
    var role_1: i32;
    var slot_8: i32;
    var local_59: i32;
    var t_21: vec4<f32>;

    x_71 = x_70;
    y_63 = y_62;
    z_71 = z_70;
    v_47 = v_46;
    top_8 = top_7;
    dx_37 = dx_36;
    dy_27 = dy_26;
    let _e195: vec4<u32> = v_47;
    let _e196: bool = cordCell(_e195);
    if _e196 {
        {
            let _e197: i32 = x_71;
            let _e198: i32 = y_63;
            let _e199: i32 = z_71;
            let _e200: bool = top_8;
            let _e201: i32 = dx_37;
            let _e202: i32 = dy_27;
            let _e203: bool = cordShows(_e197, _e198, _e199, _e200, _e201, _e202);
            return _e203;
        }
    }
    let _e204: vec4<u32> = v_47;
    let _e205: bool = top_8;
    let _e206: i32 = speciesSlot(_e204, _e205);
    own_2 = _e206;
    let _e208: i32 = own_2;
    if (_e208 >= 0i) {
        {
            let _e211: i32 = own_2;
            let _e212: i32 = x_71;
            let _e213: i32 = y_63;
            let _e214: i32 = z_71;
            let _e215: bool = top_8;
            if _e215 {
                local_58 = 1i;
            } else {
                local_58 = 0i;
            }
            let _e219: i32 = local_58;
            let _e220: i32 = dx_37;
            let _e221: i32 = dy_27;
            let _e222: vec4<f32> = faceTexel(_e211, _e212, _e213, _e214, _e219, _e220, _e221);
            t_20 = _e222;
            let _e225: vec4<u32> = v_47;
            let _e230: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e225.w)), 0i);
            role_1 = i32((_e230.w + 0.5f));
            let _e236: i32 = role_1;
            let _e241: i32 = role_1;
            let _e244: vec4<f32> = t_20;
            if (((_e236 >= 2i) && (_e241 <= ROLE_DRAPE)) && (_e244.w < 0.5f)) {
                {
                    return false;
                }
            }
            let _e250: vec4<f32> = t_20;
            let _e252: vec3<f32> = srgbToLinear(_e250.xyz);
            (*base_8) = _e252;
            return true;
        }
    }
    let _e254: vec4<u32> = v_47;
    let _e255: bool = top_8;
    let _e256: i32 = roleSlot(_e254, _e255);
    slot_8 = _e256;
    let _e258: i32 = slot_8;
    let _e259: bool = texOn(_e258);
    if !(_e259) {
        {
            return true;
        }
    }
    let _e262: i32 = slot_8;
    let _e263: i32 = x_71;
    let _e264: i32 = y_63;
    let _e265: i32 = z_71;
    let _e266: bool = top_8;
    if _e266 {
        local_59 = 1i;
    } else {
        local_59 = 0i;
    }
    let _e270: i32 = local_59;
    let _e271: i32 = dx_37;
    let _e272: i32 = dy_27;
    let _e273: vec4<f32> = faceTexel(_e262, _e263, _e264, _e265, _e270, _e271, _e272);
    t_21 = _e273;
    let _e275: i32 = slot_8;
    let _e280: vec4<f32> = t_21;
    if ((_e275 > 8i) && (_e280.w < 0.5f)) {
        {
            return false;
        }
    }
    let _e286: vec3<f32> = (*base_8);
    let _e287: vec4<f32> = t_21;
    (*base_8) = (_e286 * (_e287.x * 1.9921875f));
    return true;
}

fn glyphFrontAt(x_72: i32, y_64: i32, z_72: i32, v_48: vec4<u32>, row_5: i32, dx_38: i32, rgb: ptr<function, vec3<f32>>, glow: ptr<function, vec3<f32>>) -> bool {
    var x_73: i32;
    var y_65: i32;
    var z_73: i32;
    var v_49: vec4<u32>;
    var row_6: i32;
    var dx_39: i32;
    var q_4: u32;
    var pigment: vec4<f32>;
    var base_9: vec3<f32>;
    var tone: i32;
    var top_9: bool;
    var local_60: i32;
    var localY: i32;
    var local_61: f32;
    var hz_4: f32;
    var eAdd: vec3<f32> = vec3(0f);
    var keep: f32 = 1f;
    var e_5: vec3<f32>;
    var on_2: f32 = 1f;
    var coveredUp: bool;
    var animal: bool;
    var local_62: f32;
    var cf: f32;
    var local_63: f32;
    var ct: f32;
    var rim: bool;
    var tl: vec3<f32> = vec3(0f);
    var local_64: i32;
    var c_21: vec3<f32>;
    var gain: f32;
    var f: vec3<f32>;
    var local_65: vec3<f32>;
    var c_22: vec3<f32>;
    var gain_1: f32;
    var coveredUp_1: bool;

    x_73 = x_72;
    y_65 = y_64;
    z_73 = z_72;
    v_49 = v_48;
    row_6 = row_5;
    dx_39 = dx_38;
    (*glow) = vec3(0f);
    let _e196: vec4<u32> = v_49;
    let _e197: i32 = dx_39;
    let _e198: i32 = row_6;
    let _e199: u32 = cordGlyphTexel(_e196, _e197, _e198);
    q_4 = _e199;
    let _e201: u32 = q_4;
    let _e205: vec4<u32> = v_49;
    let _e210: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e201 & 3u)), i32(_e205.w)), 0i);
    pigment = _e210;
    let _e212: vec4<f32> = pigment;
    base_9 = _e212.xyz;
    let _e215: u32 = q_4;
    tone = i32((_e215 >> 2u));
    let _e221: i32 = row_6;
    let _e222: i32 = S;
    top_9 = (POSE && (_e221 >= _e222));
    let _e226: bool = top_9;
    if _e226 {
        let _e227: i32 = row_6;
        let _e228: i32 = S;
        local_60 = (_e227 - _e228);
    } else {
        let _e230: i32 = row_6;
        local_60 = _e230;
    }
    let _e232: i32 = local_60;
    localY = _e232;
    let _e234: i32 = z_73;
    let _e236: bool = top_9;
    if _e236 {
        let _e237: i32 = RISE;
        let _e240: i32 = localY;
        let _e243: i32 = RISE;
        local_61 = (f32(((_e237 - 1i) - _e240)) / f32(_e243));
    } else {
        local_61 = 0f;
    }
    let _e248: f32 = local_61;
    let _e250: f32 = hazeAt((f32(_e234) + _e248));
    hz_4 = _e250;
    let _e252: i32 = tone;
    let _e255: bool = top_9;
    if ((_e252 == 63i) && !(_e255)) {
        {
            return false;
        }
    }
    let _e264: i32 = tone;
    let _e266: vec4<f32> = pigment;
    let _e268: bool = emitsWhole(_e266.w);
    if (LIT && ((_e264 == TONE_EMIT) || _e268)) {
        {
            let _e271: vec4<u32> = v_49;
            let _e274: vec3<f32> = styleEmit(i32(_e271.w));
            e_5 = _e274;
            let _e278: i32 = x_73;
            let _e279: i32 = y_65;
            let _e280: i32 = z_73;
            let _e285: bool = cellEmit(_e278, _e279, _e280, (&e_5), (&on_2));
            let _e286: vec3<f32> = e_5;
            if any((_e286 > vec3(0f))) {
                {
                    let _e291: vec3<f32> = e_5;
                    let _e292: f32 = hz_4;
                    let _e293: vec3<f32> = hazed(_e291, _e292);
                    (*glow) = _e293;
                    let _e294: f32 = on_2;
                    if (_e294 >= 1f) {
                        {
                            let _e297: vec3<f32> = (*glow);
                            (*rgb) = _e297;
                            return true;
                        }
                    }
                    let _e299: vec3<f32> = e_5;
                    eAdd = _e299;
                    let _e301: f32 = on_2;
                    keep = (1f - _e301);
                }
            }
        }
    }
    let _e303: i32 = x_73;
    let _e304: i32 = y_65;
    let _e305: i32 = z_73;
    let _e306: vec4<u32> = v_49;
    let _e307: bool = top_9;
    let _e308: i32 = dx_39;
    let _e309: i32 = localY;
    let _e312: bool = plantTexel(_e303, _e304, _e305, _e306, _e307, _e308, _e309, (&base_9));
    if !(_e312) {
        {
            return false;
        }
    }
    if LIT {
        {
            let _e315: bool = top_9;
            let _e317: i32 = x_73;
            let _e318: i32 = y_65;
            let _e321: i32 = z_73;
            let _e322: bool = solidAt(_e317, (_e318 + 1i), _e321);
            let _e323: i32 = y_65;
            let _e326: bool = inY((_e323 + 1i));
            let _e327: i32 = x_73;
            let _e328: i32 = y_65;
            let _e331: i32 = z_73;
            let _e332: vec4<u32> = at(_e327, (_e328 + 1i), _e331);
            let _e333: i32 = partOf(_e332);
            let _e334: bool = isBlockPart(_e333);
            let _e336: i32 = x_73;
            let _e337: i32 = y_65;
            let _e340: i32 = z_73;
            let _e341: vec4<u32> = at(_e336, (_e337 + 1i), _e340);
            let _e342: bool = softMark(_e341);
            coveredUp = (!(_e315) && (_e322 || ((_e326 && _e334) && !(_e342))));
            let _e348: vec4<u32> = v_49;
            let _e349: i32 = partOf(_e348);
            animal = (_e349 == ANIMAL_INTERIM);
            let _e352: bool = animal;
            if _e352 {
                let _e353: i32 = x_73;
                let _e354: i32 = z_73;
                let _e357: i32 = y_65;
                let _e358: f32 = canopyAt(_e353, (_e354 - 1i), _e357);
                local_62 = _e358;
            } else {
                let _e359: vec4<u32> = v_49;
                let _e360: f32 = plantCanopy(_e359);
                local_62 = _e360;
            }
            let _e362: f32 = local_62;
            cf = _e362;
            let _e364: bool = animal;
            if _e364 {
                let _e365: i32 = x_73;
                let _e366: i32 = z_73;
                let _e367: i32 = y_65;
                let _e368: f32 = canopyAt(_e365, _e366, _e367);
                local_63 = _e368;
            } else {
                let _e369: vec4<u32> = v_49;
                let _e370: f32 = plantCanopy(_e369);
                local_63 = _e370;
            }
            let _e372: f32 = local_63;
            ct = _e372;
            let _e374: bool = top_9;
            let _e376: i32 = tone;
            let _e380: bool = coveredUp;
            rim = ((!(_e374) && (_e376 == 2i)) && !(_e380));
            let _e387: bool = top_9;
            let _e388: bool = rim;
            let _e390: i32 = tone;
            if ((_e387 || _e388) || (_e390 == 3i)) {
                {
                    let _e394: vec3<f32> = base_9;
                    let _e395: i32 = x_73;
                    let _e396: i32 = y_65;
                    let _e397: i32 = z_73;
                    let _e398: i32 = dx_39;
                    let _e399: bool = top_9;
                    if _e399 {
                        let _e400: i32 = localY;
                        local_64 = _e400;
                    } else {
                        let _e401: i32 = RISE;
                        local_64 = (_e401 - 1i);
                    }
                    let _e405: i32 = local_64;
                    let _e406: f32 = ct;
                    let _e407: vec3<f32> = shadeTop(_e394, _e395, _e396, _e397, _e398, _e405, _e406);
                    tl = _e407;
                }
            }
            let _e408: vec3<f32> = tl;
            c_21 = _e408;
            let _e410: bool = top_9;
            if _e410 {
                {
                    let _e411: i32 = tone;
                    if (_e411 == 1i) {
                        {
                            let _e414: vec3<f32> = c_21;
                            c_21 = (_e414 * u.plantA.w);
                        }
                    } else {
                        let _e419: i32 = tone;
                        let _e422: i32 = tone;
                        if ((_e419 >= 32i) && (_e422 <= 47i)) {
                            {
                                let _e427: i32 = tone;
                                gain = (0.5f + (f32((_e427 & 15i)) / 16f));
                                let _e435: vec3<f32> = c_21;
                                let _e436: vec3<f32> = tl;
                                let _e442: f32 = gain;
                                c_21 = mix(_e435, ((_e436 / vec3(u.plantA.x)) * _e442), vec3(u.shadeB.y));
                            }
                        }
                    }
                }
            } else {
                let _e449: i32 = tone;
                if (_e449 != 3i) {
                    {
                        let _e452: i32 = tone;
                        if (_e452 == 1i) {
                            {
                                let _e455: vec3<f32> = base_9;
                                base_9 = (_e455 * u.shadeB.x);
                            }
                        } else {
                            let _e460: i32 = tone;
                            let _e463: i32 = tone;
                            if ((_e460 == 4i) || (_e463 == 5i)) {
                                {
                                    let _e467: vec3<f32> = base_9;
                                    let _e468: vec4<u32> = v_49;
                                    let _e472: vec3<f32> = styleAt(i32(_e468.w), 0i);
                                    base_9 = mix(_e467, _e472, vec3(u.plantB.x));
                                    let _e478: i32 = tone;
                                    if (_e478 == 5i) {
                                        {
                                            let _e481: vec3<f32> = base_9;
                                            let _e486: i32 = x_73;
                                            let _e487: i32 = y_65;
                                            let _e488: i32 = z_73;
                                            let _e489: i32 = dx_39;
                                            let _e490: f32 = crownEdge(_e486, _e487, _e488, _e489);
                                            base_9 = (_e481 * mix(1f, u.plantA.w, _e490));
                                        }
                                    }
                                }
                            } else {
                                let _e493: i32 = tone;
                                let _e496: i32 = tone;
                                if ((_e493 >= 16i) && (_e496 <= 31i)) {
                                    {
                                        let _e500: vec3<f32> = base_9;
                                        let _e502: i32 = tone;
                                        base_9 = (_e500 * (0.5f + (f32((_e502 & 15i)) / 16f)));
                                    }
                                }
                            }
                        }
                        let _e510: vec3<f32> = base_9;
                        let _e511: i32 = x_73;
                        let _e512: i32 = y_65;
                        let _e513: i32 = z_73;
                        let _e514: i32 = dx_39;
                        let _e515: i32 = localY;
                        let _e516: f32 = cf;
                        let _e517: vec3<f32> = shadeFront(_e510, _e511, _e512, _e513, _e514, _e515, _e516);
                        f = _e517;
                        let _e519: bool = rim;
                        if _e519 {
                            let _e520: vec3<f32> = f;
                            let _e521: vec3<f32> = tl;
                            local_65 = mix(_e520, _e521, vec3(u.plantA.z));
                        } else {
                            let _e527: vec3<f32> = f;
                            local_65 = _e527;
                        }
                        let _e529: vec3<f32> = local_65;
                        c_21 = _e529;
                    }
                }
            }
            let _e530: vec3<f32> = c_21;
            let _e531: f32 = keep;
            let _e533: vec3<f32> = eAdd;
            let _e535: f32 = hz_4;
            let _e536: vec3<f32> = hazed(((_e530 * _e531) + _e533), _e535);
            (*rgb) = _e536;
            return true;
        }
    }
    let _e538: bool = top_9;
    if _e538 {
        {
            let _e539: vec3<f32> = base_9;
            let _e540: i32 = x_73;
            let _e541: i32 = y_65;
            let _e542: i32 = z_73;
            let _e543: i32 = roofGap(_e540, _e541, _e542);
            let _e544: f32 = roofShade(_e543);
            let _e545: vec3<f32> = plantLit(_e539, _e544);
            c_22 = _e545;
            let _e547: i32 = tone;
            if (_e547 == 1i) {
                {
                    let _e550: vec3<f32> = c_22;
                    c_22 = (_e550 * u.plantA.w);
                }
            } else {
                let _e555: i32 = tone;
                let _e558: i32 = tone;
                if ((_e555 >= 32i) && (_e558 <= 47i)) {
                    {
                        let _e563: i32 = tone;
                        gain_1 = (0.5f + (f32((_e563 & 15i)) / 16f));
                        let _e571: vec3<f32> = c_22;
                        let _e572: vec3<f32> = base_9;
                        let _e573: f32 = gain_1;
                        c_22 = mix(_e571, (_e572 * _e573), vec3(u.shadeB.y));
                    }
                }
            }
            let _e580: vec3<f32> = c_22;
            let _e581: f32 = hz_4;
            let _e582: vec3<f32> = hazed(_e580, _e581);
            (*rgb) = _e582;
            return true;
        }
    }
    let _e584: i32 = x_73;
    let _e585: i32 = y_65;
    let _e588: i32 = z_73;
    let _e589: bool = solidAt(_e584, (_e585 + 1i), _e588);
    let _e590: i32 = y_65;
    let _e593: bool = inY((_e590 + 1i));
    let _e594: i32 = x_73;
    let _e595: i32 = y_65;
    let _e598: i32 = z_73;
    let _e599: vec4<u32> = at(_e594, (_e595 + 1i), _e598);
    let _e600: i32 = partOf(_e599);
    let _e601: bool = isBlockPart(_e600);
    let _e603: i32 = x_73;
    let _e604: i32 = y_65;
    let _e607: i32 = z_73;
    let _e608: vec4<u32> = at(_e603, (_e604 + 1i), _e607);
    let _e609: bool = softMark(_e608);
    coveredUp_1 = (_e589 || ((_e593 && _e601) && !(_e609)));
    let _e614: i32 = tone;
    if (_e614 == 1i) {
        {
            let _e617: vec3<f32> = base_9;
            base_9 = (_e617 * u.shadeB.x);
        }
    } else {
        let _e622: i32 = tone;
        let _e625: bool = coveredUp_1;
        if ((_e622 == 2i) && !(_e625)) {
            {
                let _e628: vec3<f32> = base_9;
                let _e629: vec3<f32> = base_9;
                let _e630: i32 = x_73;
                let _e631: i32 = y_65;
                let _e632: i32 = z_73;
                let _e633: i32 = roofGap(_e630, _e631, _e632);
                let _e634: f32 = roofShade(_e633);
                let _e635: vec3<f32> = plantLit(_e629, _e634);
                base_9 = mix(_e628, _e635, vec3(u.plantA.z));
            }
        } else {
            let _e641: i32 = tone;
            if (_e641 == 3i) {
                {
                    let _e644: vec3<f32> = base_9;
                    let _e645: i32 = x_73;
                    let _e646: i32 = y_65;
                    let _e647: i32 = z_73;
                    let _e648: i32 = roofGap(_e645, _e646, _e647);
                    let _e649: f32 = roofShade(_e648);
                    let _e650: vec3<f32> = plantLit(_e644, _e649);
                    base_9 = _e650;
                }
            } else {
                let _e651: i32 = tone;
                let _e654: i32 = tone;
                if ((_e651 == 4i) || (_e654 == 5i)) {
                    {
                        let _e658: vec3<f32> = base_9;
                        let _e659: vec4<u32> = v_49;
                        let _e663: vec3<f32> = styleAt(i32(_e659.w), 0i);
                        base_9 = mix(_e658, _e663, vec3(u.plantB.x));
                        let _e669: i32 = tone;
                        if (_e669 == 5i) {
                            {
                                let _e672: vec3<f32> = base_9;
                                let _e677: i32 = x_73;
                                let _e678: i32 = y_65;
                                let _e679: i32 = z_73;
                                let _e680: i32 = dx_39;
                                let _e681: f32 = crownEdge(_e677, _e678, _e679, _e680);
                                base_9 = (_e672 * mix(1f, u.plantA.w, _e681));
                            }
                        }
                    }
                } else {
                    let _e684: i32 = tone;
                    let _e687: i32 = tone;
                    if ((_e684 >= 16i) && (_e687 <= 31i)) {
                        {
                            let _e691: vec3<f32> = base_9;
                            let _e693: i32 = tone;
                            base_9 = (_e691 * (0.5f + (f32((_e693 & 15i)) / 16f)));
                        }
                    }
                }
            }
        }
    }
    let _e701: vec3<f32> = base_9;
    let _e702: f32 = hz_4;
    let _e703: vec3<f32> = hazed(_e701, _e702);
    (*rgb) = _e703;
    return true;
}

fn glyphCapAt(x_74: i32, y_66: i32, z_74: i32, v_50: vec4<u32>, localY_1: i32, dx_40: i32, rgb_1: ptr<function, vec3<f32>>, glow_1: ptr<function, vec3<f32>>) -> bool {
    var x_75: i32;
    var y_67: i32;
    var z_75: i32;
    var v_51: vec4<u32>;
    var localY_2: i32;
    var dx_41: i32;
    var r_14: i32;
    var atlasY: i32;
    var q_5: u32;
    var pigment_1: vec4<f32>;
    var base_10: vec3<f32>;
    var tone_1: i32;
    var eAdd_1: vec3<f32> = vec3(0f);
    var keep_1: f32 = 1f;
    var e_6: vec3<f32>;
    var on_3: f32 = 1f;
    var local_66: f32;
    var ct_1: f32;
    var own_3: vec3<f32>;
    var lc: vec3<f32>;
    var gain_2: f32;
    var shade_4: f32;
    var cap_1: vec3<f32>;
    var gain_3: f32;

    x_75 = x_74;
    y_67 = y_66;
    z_75 = z_74;
    v_51 = v_50;
    localY_2 = localY_1;
    dx_41 = dx_40;
    (*glow_1) = vec3(0f);
    let _e196: i32 = RISE;
    let _e199: i32 = localY_2;
    r_14 = ((_e196 - 1i) - _e199);
    let _e202: vec4<u32> = v_51;
    let _e203: i32 = partOf(_e202);
    let _e206: vec4<u32> = v_51;
    let _e207: i32 = glyphOf(_e206);
    let _e209: i32 = S;
    let _e210: i32 = RISE;
    let _e213: i32 = S;
    let _e215: i32 = localY_2;
    atlasY = (((((_e203 * 8i) + _e207) * (_e209 + _e210)) + _e213) + _e215);
    let _e218: i32 = dx_41;
    let _e219: i32 = atlasY;
    let _e222: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e218, _e219), 0i);
    q_5 = _e222.x;
    let _e225: u32 = q_5;
    let _e229: vec4<u32> = v_51;
    let _e234: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e225 & 3u)), i32(_e229.w)), 0i);
    pigment_1 = _e234;
    let _e236: vec4<f32> = pigment_1;
    base_10 = _e236.xyz;
    let _e239: u32 = q_5;
    tone_1 = i32((_e239 >> 2u));
    let _e250: i32 = tone_1;
    let _e252: vec4<f32> = pigment_1;
    let _e254: bool = emitsWhole(_e252.w);
    if (LIT && ((_e250 == TONE_EMIT) || _e254)) {
        {
            let _e257: vec4<u32> = v_51;
            let _e260: vec3<f32> = styleEmit(i32(_e257.w));
            e_6 = _e260;
            let _e264: i32 = x_75;
            let _e265: i32 = y_67;
            let _e266: i32 = z_75;
            let _e271: bool = cellEmit(_e264, _e265, _e266, (&e_6), (&on_3));
            let _e272: vec3<f32> = e_6;
            if any((_e272 > vec3(0f))) {
                {
                    let _e277: vec3<f32> = e_6;
                    let _e278: i32 = z_75;
                    let _e280: i32 = r_14;
                    let _e282: i32 = RISE;
                    let _e286: f32 = hazeAt((f32(_e278) + (f32(_e280) / f32(_e282))));
                    let _e287: vec3<f32> = hazed(_e277, _e286);
                    (*glow_1) = _e287;
                    let _e288: f32 = on_3;
                    if (_e288 >= 1f) {
                        {
                            let _e291: vec3<f32> = (*glow_1);
                            (*rgb_1) = _e291;
                            return true;
                        }
                    }
                    let _e293: vec3<f32> = e_6;
                    eAdd_1 = _e293;
                    let _e295: f32 = on_3;
                    keep_1 = (1f - _e295);
                }
            }
        }
    }
    let _e297: i32 = x_75;
    let _e298: i32 = y_67;
    let _e299: i32 = z_75;
    let _e300: vec4<u32> = v_51;
    let _e302: i32 = dx_41;
    let _e303: i32 = localY_2;
    let _e306: bool = plantTexel(_e297, _e298, _e299, _e300, true, _e302, _e303, (&base_10));
    if !(_e306) {
        {
            return false;
        }
    }
    if LIT {
        {
            let _e309: vec4<u32> = v_51;
            let _e310: i32 = partOf(_e309);
            if (_e310 == ANIMAL_INTERIM) {
                let _e312: i32 = x_75;
                let _e313: i32 = z_75;
                let _e314: i32 = y_67;
                let _e315: f32 = canopyAt(_e312, _e313, _e314);
                local_66 = _e315;
            } else {
                let _e316: vec4<u32> = v_51;
                let _e317: f32 = plantCanopy(_e316);
                local_66 = _e317;
            }
            let _e319: f32 = local_66;
            ct_1 = _e319;
            let _e321: vec3<f32> = base_10;
            let _e322: i32 = x_75;
            let _e323: i32 = y_67;
            let _e324: i32 = z_75;
            let _e325: i32 = dx_41;
            let _e326: i32 = localY_2;
            let _e327: f32 = ct_1;
            let _e328: vec3<f32> = shadeTop(_e321, _e322, _e323, _e324, _e325, _e326, _e327);
            own_3 = _e328;
            let _e330: vec3<f32> = own_3;
            lc = _e330;
            let _e332: i32 = tone_1;
            if (_e332 == 1i) {
                {
                    let _e335: vec3<f32> = lc;
                    let _e340: i32 = x_75;
                    let _e341: i32 = y_67;
                    let _e342: i32 = z_75;
                    let _e343: i32 = dx_41;
                    let _e344: f32 = crownEdge(_e340, _e341, _e342, _e343);
                    lc = (_e335 * mix(1f, u.plantA.w, _e344));
                }
            } else {
                let _e347: i32 = tone_1;
                let _e350: i32 = tone_1;
                if ((_e347 >= 32i) && (_e350 <= 47i)) {
                    {
                        let _e355: i32 = tone_1;
                        gain_2 = (0.5f + (f32((_e355 & 15i)) / 16f));
                        let _e363: vec3<f32> = lc;
                        let _e364: vec3<f32> = own_3;
                        let _e370: f32 = gain_2;
                        lc = mix(_e363, ((_e364 / vec3(u.plantA.x)) * _e370), vec3(u.shadeB.y));
                    }
                }
            }
            let _e377: vec3<f32> = lc;
            let _e378: f32 = keep_1;
            let _e380: vec3<f32> = eAdd_1;
            let _e382: i32 = z_75;
            let _e384: i32 = r_14;
            let _e386: i32 = RISE;
            let _e390: f32 = hazeAt((f32(_e382) + (f32(_e384) / f32(_e386))));
            let _e391: vec3<f32> = hazed(((_e377 * _e378) + _e380), _e390);
            (*rgb_1) = _e391;
            return true;
        }
    }
    let _e393: i32 = x_75;
    let _e394: i32 = y_67;
    let _e395: i32 = z_75;
    let _e396: i32 = roofGap(_e393, _e394, _e395);
    let _e397: f32 = roofShade(_e396);
    shade_4 = _e397;
    let _e399: vec3<f32> = base_10;
    let _e400: f32 = shade_4;
    let _e401: vec3<f32> = plantLit(_e399, _e400);
    cap_1 = _e401;
    let _e403: i32 = tone_1;
    if (_e403 == 1i) {
        {
            let _e406: vec3<f32> = cap_1;
            let _e411: i32 = x_75;
            let _e412: i32 = y_67;
            let _e413: i32 = z_75;
            let _e414: i32 = dx_41;
            let _e415: f32 = crownEdge(_e411, _e412, _e413, _e414);
            cap_1 = (_e406 * mix(1f, u.plantA.w, _e415));
        }
    } else {
        let _e418: i32 = tone_1;
        let _e421: i32 = tone_1;
        if ((_e418 >= 32i) && (_e421 <= 47i)) {
            {
                let _e426: i32 = tone_1;
                gain_3 = (0.5f + (f32((_e426 & 15i)) / 16f));
                let _e434: vec3<f32> = cap_1;
                let _e435: vec3<f32> = base_10;
                let _e436: f32 = gain_3;
                cap_1 = mix(_e434, (_e435 * _e436), vec3(u.shadeB.y));
            }
        }
    }
    let _e443: vec3<f32> = cap_1;
    let _e444: i32 = z_75;
    let _e446: i32 = r_14;
    let _e448: i32 = RISE;
    let _e452: f32 = hazeAt((f32(_e444) + (f32(_e446) / f32(_e448))));
    let _e453: vec3<f32> = hazed(_e443, _e452);
    (*rgb_1) = _e453;
    return true;
}

fn plantHidden(v_52: vec4<u32>) -> bool {
    var v_53: vec4<u32>;

    v_53 = v_52;
    let _e182: vec4<u32> = v_53;
    let _e187: vec4<u32> = v_53;
    let _e188: i32 = partOf(_e187);
    let _e189: bool = plantBlockPart(_e188);
    return ((PLANT_MODELS && (_e182.y == 0u)) && _e189);
}

fn isPosed(v_54: vec4<u32>) -> bool {
    var v_55: vec4<u32>;

    v_55 = v_54;
    let _e182: vec4<u32> = v_55;
    let _e183: i32 = partOf(_e182);
    let _e186: vec4<u32> = v_55;
    return ((POSE && (_e183 == ANIMAL_INTERIM)) && (_e186.z != 0u));
}

fn posedSpan(c_23: vec4<u32>, sy: i32) -> vec2<i32> {
    var c_24: vec4<u32>;
    var sy_1: i32;
    var cls: i32;
    var db: vec2<i32>;
    var local_67: i32;

    c_24 = c_23;
    sy_1 = sy;
    let _e184: vec4<u32> = c_24;
    cls = i32((_e184.z >> 6u));
    let _e191: vec4<u32> = c_24;
    let _e198: vec4<f32> = u.poseK[i32((_e191.z & 63u))];
    db = vec2<i32>(_e198.xy);
    let _e202: i32 = cls;
    if (_e202 >= 2i) {
        {
            let _e205: i32 = sy_1;
            let _e206: i32 = S;
            let _e208: i32 = sy_1;
            let _e209: i32 = S;
            let _e211: i32 = sy_1;
            let _e214: i32 = S;
            let _e216: i32 = sy_1;
            let _e217: i32 = cls;
            let _e221: i32 = S;
            let _e223: vec2<i32> = db;
            return vec2<i32>((_e205 * _e206), max((_e208 * _e209), min(((_e211 + 1i) * _e214), ((((_e216 + _e217) - 1i) * _e221) - _e223.x))));
        }
    }
    let _e229: i32 = sy_1;
    let _e230: i32 = S;
    let _e232: vec2<i32> = db;
    let _e235: i32 = sy_1;
    let _e238: i32 = S;
    let _e240: vec2<i32> = db;
    let _e243: i32 = cls;
    if (_e243 == 1i) {
        let _e246: vec2<i32> = db;
        local_67 = _e246.y;
    } else {
        local_67 = 0i;
    }
    let _e250: i32 = local_67;
    return vec2<i32>(((_e229 * _e230) - _e232.x), ((((_e235 + 1i) * _e238) - _e240.x) - _e250));
}

fn posedRow(c_25: vec4<u32>, sy_2: i32, sp: vec2<i32>, q_6: i32) -> i32 {
    var c_26: vec4<u32>;
    var sy_3: i32;
    var sp_1: vec2<i32>;
    var q_7: i32;
    var t_22: i32;
    var local_68: i32;

    c_26 = c_25;
    sy_3 = sy_2;
    sp_1 = sp;
    q_7 = q_6;
    let _e188: vec4<u32> = c_26;
    if (i32((_e188.z >> 6u)) >= 2i) {
        {
            let _e196: i32 = sy_3;
            let _e199: i32 = S;
            let _e203: i32 = q_7;
            return ((((_e196 + 1i) * _e199) - 1i) - _e203);
        }
    }
    let _e205: vec2<i32> = sp_1;
    let _e209: i32 = q_7;
    t_22 = ((_e205.y - 1i) - _e209);
    let _e212: i32 = t_22;
    if (_e212 > 0i) {
        let _e215: i32 = t_22;
        let _e216: i32 = S;
        let _e218: vec2<i32> = sp_1;
        let _e220: vec2<i32> = sp_1;
        local_68 = ((_e215 + _e216) - (_e218.y - _e220.x));
    } else {
        local_68 = 0i;
    }
    let _e226: i32 = local_68;
    return _e226;
}

fn posedFind(x_76: i32, level: i32, z_76: i32, r_15: i32, gy: ptr<function, i32>, row_7: ptr<function, i32>) -> bool {
    var x_77: i32;
    var level_1: i32;
    var z_77: i32;
    var r_16: i32;
    var q_8: i32;
    var i_8: i32 = 0i;
    var sy_4: i32;
    var c_27: vec4<u32>;
    var sp_2: vec2<i32>;

    x_77 = x_76;
    level_1 = level;
    z_77 = z_76;
    r_16 = r_15;
    let _e190: i32 = level_1;
    let _e191: i32 = S;
    let _e193: i32 = r_16;
    q_8 = ((_e190 * _e191) + _e193);
    let _e196: i32 = level_1;
    (*gy) = _e196;
    (*row_7) = -1i;
    loop {
        let _e201: i32 = i_8;
        if !((_e201 < 3i)) {
            break;
        }
        {
            let _e208: i32 = level_1;
            let _e211: i32 = i_8;
            sy_4 = ((_e208 + 2i) - _e211);
            let _e214: i32 = sy_4;
            let _e215: bool = inY(_e214);
            if !(_e215) {
                {
                    continue;
                }
            }
            let _e217: i32 = x_77;
            let _e218: i32 = sy_4;
            let _e219: i32 = z_77;
            let _e220: vec4<u32> = at(_e217, _e218, _e219);
            c_27 = _e220;
            let _e222: vec4<u32> = c_27;
            let _e223: bool = isPosed(_e222);
            let _e225: vec4<u32> = c_27;
            let _e226: i32 = glyphOf(_e225);
            if (!(_e223) || (_e226 == 0i)) {
                {
                    continue;
                }
            }
            let _e230: vec4<u32> = c_27;
            let _e231: i32 = sy_4;
            let _e232: vec2<i32> = posedSpan(_e230, _e231);
            sp_2 = _e232;
            let _e234: vec2<i32> = sp_2;
            let _e236: vec2<i32> = sp_2;
            if (_e234.y <= _e236.x) {
                {
                    continue;
                }
            }
            let _e239: i32 = q_8;
            let _e240: vec2<i32> = sp_2;
            let _e243: i32 = q_8;
            let _e244: vec2<i32> = sp_2;
            if ((_e239 >= _e240.x) && (_e243 < _e244.y)) {
                {
                    let _e248: i32 = sy_4;
                    (*gy) = _e248;
                    let _e249: vec4<u32> = c_27;
                    let _e250: i32 = sy_4;
                    let _e251: vec2<i32> = sp_2;
                    let _e252: i32 = q_8;
                    let _e253: i32 = posedRow(_e249, _e250, _e251, _e252);
                    (*row_7) = _e253;
                    return true;
                }
            }
            let _e255: i32 = (*row_7);
            let _e258: i32 = q_8;
            let _e259: vec2<i32> = sp_2;
            let _e263: i32 = q_8;
            let _e264: vec2<i32> = sp_2;
            let _e266: i32 = RISE;
            if (((_e255 < 0i) && (_e258 >= _e259.y)) && (_e263 < (_e264.y + _e266))) {
                {
                    let _e270: i32 = sy_4;
                    (*gy) = _e270;
                    let _e271: i32 = S;
                    let _e272: i32 = RISE;
                    let _e276: i32 = q_8;
                    let _e277: vec2<i32> = sp_2;
                    (*row_7) = (((_e271 + _e272) - 1i) - (_e276 - _e277.y));
                }
            }
        }
        continuing {
            let _e205: i32 = i_8;
            i_8 = (_e205 + 1i);
        }
    }
    let _e281: i32 = (*row_7);
    return (_e281 >= 0i);
}

fn posedCapRow(v_56: vec4<u32>, level_2: i32, r_17: i32) -> i32 {
    var v_57: vec4<u32>;
    var level_3: i32;
    var r_18: i32;
    var sp_3: vec2<i32>;
    var q_9: i32;
    var local_69: i32;

    v_57 = v_56;
    level_3 = level_2;
    r_18 = r_17;
    let _e186: vec4<u32> = v_57;
    let _e187: i32 = glyphOf(_e186);
    if (_e187 == 0i) {
        {
            return -1i;
        }
    }
    let _e192: vec4<u32> = v_57;
    let _e193: i32 = level_3;
    let _e196: vec2<i32> = posedSpan(_e192, (_e193 - 1i));
    sp_3 = _e196;
    let _e198: vec2<i32> = sp_3;
    let _e200: vec2<i32> = sp_3;
    if (_e198.y <= _e200.x) {
        {
            return -1i;
        }
    }
    let _e205: i32 = level_3;
    let _e206: i32 = S;
    let _e208: i32 = r_18;
    q_9 = ((_e205 * _e206) + _e208);
    let _e211: i32 = q_9;
    let _e212: vec2<i32> = sp_3;
    let _e215: i32 = q_9;
    let _e216: vec2<i32> = sp_3;
    let _e218: i32 = RISE;
    if ((_e211 >= _e212.y) && (_e215 < (_e216.y + _e218))) {
        let _e222: i32 = RISE;
        let _e225: i32 = q_9;
        let _e226: vec2<i32> = sp_3;
        local_69 = ((_e222 - 1i) - (_e225 - _e226.y));
    } else {
        local_69 = -1i;
    }
    let _e233: i32 = local_69;
    return _e233;
}

fn nearerOwns(x_78: i32, y_68: i32, z_78: i32, row_8: i32) -> bool {
    var x_79: i32;
    var y_69: i32;
    var z_79: i32;
    var row_9: i32;
    var R: i32;
    var dy_28: i32 = 0i;
    var yn: i32;
    var f_1: i32;
    var top_10: i32;
    var fill: i32;
    var skin: i32;
    var local_70: i32;

    x_79 = x_78;
    y_69 = y_68;
    z_79 = z_78;
    row_9 = row_8;
    let _e188: i32 = z_79;
    if (_e188 == 0i) {
        {
            return false;
        }
    }
    let _e192: i32 = y_69;
    let _e193: i32 = z_79;
    let _e194: i32 = frontRow(_e192, _e193);
    R = _e194;
    loop {
        let _e198: i32 = dy_28;
        if !((_e198 < 3i)) {
            break;
        }
        {
            let _e205: i32 = y_69;
            let _e206: i32 = dy_28;
            yn = (_e205 + _e206);
            let _e209: i32 = R;
            let _e210: i32 = RISE;
            let _e212: i32 = dy_28;
            let _e213: i32 = S;
            f_1 = ((_e209 + _e210) - (_e212 * _e213));
            let _e218: i32 = x_79;
            let _e219: i32 = yn;
            let _e220: i32 = z_79;
            let _e223: bool = solidAt(_e218, _e219, (_e220 - 1i));
            if _e223 {
                {
                    let _e224: i32 = f_1;
                    let _e225: i32 = RISE;
                    top_10 = (_e224 - _e225);
                }
            } else {
                {
                    let _e227: i32 = x_79;
                    let _e228: i32 = yn;
                    let _e229: i32 = z_79;
                    let _e232: i32 = fillPxAt(_e227, _e228, (_e229 - 1i));
                    fill = _e232;
                    let _e234: i32 = fill;
                    if (_e234 == 0i) {
                        {
                            continue;
                        }
                    }
                    let _e237: i32 = f_1;
                    let _e238: i32 = S;
                    let _e240: i32 = fill;
                    skin = ((_e237 + _e238) - _e240);
                    let _e243: i32 = x_79;
                    let _e244: i32 = yn;
                    let _e245: i32 = z_79;
                    let _e248: bool = waterOpenUp(_e243, _e244, (_e245 - 1i));
                    if _e248 {
                        let _e249: i32 = skin;
                        let _e250: i32 = RISE;
                        local_70 = (_e249 - _e250);
                    } else {
                        let _e252: i32 = skin;
                        local_70 = _e252;
                    }
                    let _e254: i32 = local_70;
                    top_10 = _e254;
                }
            }
            let _e255: i32 = row_9;
            let _e256: i32 = top_10;
            let _e258: i32 = row_9;
            let _e259: i32 = f_1;
            let _e260: i32 = S;
            if ((_e255 >= _e256) && (_e258 < (_e259 + _e260))) {
                {
                    return true;
                }
            }
        }
        continuing {
            let _e202: i32 = dy_28;
            dy_28 = (_e202 + 1i);
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
    return (-(log(max(u.waterDeepC.xyz, vec3(0.00024414063f)))) * u.waterL.x);
}

fn fallingAt(x_80: i32, y_70: i32, z_80: i32) -> bool {
    var x_81: i32;
    var y_71: i32;
    var z_81: i32;

    x_81 = x_80;
    y_71 = y_70;
    z_81 = z_80;
    let _e194: i32 = y_71;
    let _e197: i32 = x_81;
    let _e198: i32 = y_71;
    let _e201: i32 = z_81;
    let _e202: bool = solidAt(_e197, (_e198 - 1i), _e201);
    let _e205: i32 = x_81;
    let _e206: i32 = y_71;
    let _e209: i32 = z_81;
    let _e210: i32 = fillPxAt(_e205, (_e206 - 1i), _e209);
    let _e211: i32 = S;
    return (((_e194 > 0i) && !(_e202)) && (_e210 < _e211));
}

fn waterAtLit(x_82: i32, y_72: i32, z_82: i32, v_58: vec4<u32>, row_10: i32, acc: ptr<function, vec3<f32>>, trans_2: ptr<function, vec3<f32>>) {
    var x_83: i32;
    var y_73: i32;
    var z_83: i32;
    var v_59: vec4<u32>;
    var row_11: i32;
    var fill_1: i32;
    var R_1: i32;
    var bottom: i32;
    var skinRow: i32;
    var openUp_2: bool;
    var topRow: bool;
    var nearSolid: bool;
    var local_71: i32;
    var nearFill: i32;
    var nearSkin: i32;
    var stop: i32;
    var nearOpenUp: bool;
    var local_72: i32;
    var bodyRow: bool;
    var k_7: f32;
    var yf_2: f32;
    var h_7: f32;
    var local_73: i32;
    var falling: bool;
    var local_74: i32;
    var ys: i32;
    var i_9: i32 = 0i;
    var local_75: i32;
    var a_11: f32;
    var alpha: f32;
    var skinAlpha: f32;
    var a_12: f32;
    var c_28: vec3<f32>;
    var hf: f32;
    var isSkin: bool;
    var local_76: f32;
    var local_77: vec3<f32>;
    var top_11: f32;
    var hi_4: f32;
    var lo_5: f32;
    var l_5: f32;
    var T: vec3<f32>;
    var depth: f32;
    var cin: vec3<f32>;

    x_83 = x_82;
    y_73 = y_72;
    z_83 = z_82;
    v_59 = v_58;
    row_11 = row_10;
    let _e200: vec4<u32> = v_59;
    let _e203: i32 = fillPxQ(i32(_e200.y));
    fill_1 = _e203;
    let _e205: i32 = y_73;
    let _e206: i32 = z_83;
    let _e207: i32 = frontRow(_e205, _e206);
    R_1 = _e207;
    let _e209: i32 = R_1;
    let _e210: i32 = S;
    bottom = (_e209 + _e210);
    let _e213: i32 = bottom;
    let _e214: i32 = fill_1;
    skinRow = (_e213 - _e214);
    let _e217: i32 = x_83;
    let _e218: i32 = y_73;
    let _e219: i32 = z_83;
    let _e220: bool = waterOpenUp(_e217, _e218, _e219);
    openUp_2 = _e220;
    let _e222: bool = openUp_2;
    let _e223: i32 = row_11;
    let _e224: i32 = skinRow;
    let _e225: i32 = RISE;
    let _e229: i32 = row_11;
    let _e230: i32 = skinRow;
    let _e233: i32 = x_83;
    let _e234: i32 = y_73;
    let _e235: i32 = z_83;
    let _e236: i32 = row_11;
    let _e237: bool = nearerOwns(_e233, _e234, _e235, _e236);
    topRow = (((_e222 && (_e223 >= (_e224 - _e225))) && (_e229 < _e230)) && !(_e237));
    let _e241: i32 = z_83;
    let _e244: i32 = x_83;
    let _e245: i32 = y_73;
    let _e246: i32 = z_83;
    let _e249: bool = solidAt(_e244, _e245, (_e246 - 1i));
    nearSolid = ((_e241 > 0i) && _e249);
    let _e252: i32 = z_83;
    if (_e252 > 0i) {
        let _e255: i32 = x_83;
        let _e256: i32 = y_73;
        let _e257: i32 = z_83;
        let _e260: i32 = fillPxAt(_e255, _e256, (_e257 - 1i));
        local_71 = _e260;
    } else {
        local_71 = 0i;
    }
    let _e263: i32 = local_71;
    nearFill = _e263;
    let _e265: i32 = R_1;
    let _e266: i32 = RISE;
    let _e268: i32 = S;
    let _e270: i32 = nearFill;
    nearSkin = (((_e265 + _e266) + _e268) - _e270);
    let _e273: i32 = bottom;
    stop = _e273;
    let _e275: bool = nearSolid;
    if _e275 {
        {
            let _e276: i32 = skinRow;
            stop = _e276;
        }
    } else {
        let _e277: i32 = nearFill;
        if (_e277 > 0i) {
            {
                let _e280: i32 = x_83;
                let _e281: i32 = y_73;
                let _e282: i32 = z_83;
                let _e285: bool = waterOpenUp(_e280, _e281, (_e282 - 1i));
                nearOpenUp = _e285;
                let _e287: i32 = bottom;
                let _e288: bool = nearOpenUp;
                if _e288 {
                    let _e289: i32 = nearSkin;
                    let _e290: i32 = RISE;
                    local_72 = (_e289 - _e290);
                } else {
                    let _e292: i32 = nearSkin;
                    local_72 = _e292;
                }
                let _e294: i32 = local_72;
                stop = min(_e287, _e294);
            }
        }
    }
    let _e296: i32 = row_11;
    let _e297: i32 = skinRow;
    let _e299: i32 = row_11;
    let _e300: i32 = stop;
    bodyRow = ((_e296 >= _e297) && (_e299 < _e300));
    let _e304: i32 = RISE;
    let _e306: i32 = S;
    k_7 = (f32(_e304) / f32(_e306));
    let _e310: i32 = BASE;
    let _e311: i32 = z_83;
    let _e312: i32 = RISE;
    let _e315: i32 = row_11;
    let _e322: i32 = S;
    yf_2 = ((f32((((_e310 - (_e311 * _e312)) - _e315) - 1i)) + 0.5f) / f32(_e322));
    let _e326: i32 = z_83;
    let _e330: f32 = hazeAt((f32(_e326) + 0.5f));
    h_7 = _e330;
    let _e332: i32 = wEntry;
    if (_e332 < 0i) {
        {
            let _e335: bool = topRow;
            let _e337: bool = bodyRow;
            if (!(_e335) && !(_e337)) {
                {
                    return;
                }
            }
            let _e340: i32 = y_73;
            let _e341: i32 = z_83;
            let _e346: bool = topRow;
            if _e346 {
                local_73 = 16777216i;
            } else {
                local_73 = 0i;
            }
            let _e353: i32 = local_73;
            wEntry = ((_e340 | (_e341 << 12u)) | _e353);
            let _e355: i32 = x_83;
            let _e356: i32 = y_73;
            let _e357: i32 = z_83;
            let _e358: bool = fallingAt(_e355, _e356, _e357);
            falling = _e358;
            let _e360: i32 = z_83;
            let _e363: bool = topRow;
            let _e366: bool = falling;
            if (((_e360 == 0i) && !(_e363)) || _e366) {
                {
                    let _e368: i32 = wEntry;
                    let _e369: bool = falling;
                    if _e369 {
                        local_74 = ENTRY_FALLING;
                    } else {
                        local_74 = 0i;
                    }
                    let _e372: i32 = local_74;
                    wEntry = (_e368 | (ENTRY_FLAT | _e372));
                }
            } else {
                {
                    let _e375: i32 = y_73;
                    ys = _e375;
                    loop {
                        let _e379: i32 = i_9;
                        let _e382: bool = topRow;
                        if !(((_e379 < 8i) && !(_e382))) {
                            break;
                        }
                        {
                            let _e389: i32 = x_83;
                            let _e390: i32 = ys;
                            let _e393: i32 = z_83;
                            let _e394: bool = solidAt(_e389, (_e390 + 1i), _e393);
                            let _e395: i32 = x_83;
                            let _e396: i32 = ys;
                            let _e399: i32 = z_83;
                            let _e400: i32 = freeQ(_e395, (_e396 + 1i), _e399);
                            if (_e394 || (_e400 == 0i)) {
                                {
                                    break;
                                }
                            }
                            let _e404: i32 = ys;
                            ys = (_e404 + 1i);
                        }
                        continuing {
                            let _e386: i32 = i_9;
                            i_9 = (_e386 + 1i);
                        }
                    }
                    let _e407: i32 = ys;
                    let _e409: bool = topRow;
                    if _e409 {
                        let _e410: i32 = fill_1;
                        local_75 = _e410;
                    } else {
                        let _e411: i32 = x_83;
                        let _e412: i32 = ys;
                        let _e413: i32 = z_83;
                        let _e414: i32 = fillPxAt(_e411, _e412, _e413);
                        local_75 = _e414;
                    }
                    let _e416: i32 = local_75;
                    let _e418: i32 = S;
                    wSurf = (f32(_e407) + (f32(_e416) / f32(_e418)));
                    let _e422: bool = bodyRow;
                    let _e423: i32 = row_11;
                    let _e424: i32 = skinRow;
                    let _e427: bool = openUp_2;
                    if ((_e422 && (_e423 == _e424)) && _e427) {
                        {
                            a_11 = min((clamp(u.knobs.y, 0f, 1f) * u.waterK.x), 0.95f);
                            let _e442: vec3<f32> = wScatter;
                            let _e443: vec3<f32> = (*trans_2);
                            let _e444: f32 = a_11;
                            let _e447: f32 = h_7;
                            wScatter = (_e442 + (((_e443 * _e444) * (1f - _e447)) * u.waterSurfaceC.xyz));
                            let _e455: f32 = wHaze;
                            let _e456: vec3<f32> = (*trans_2);
                            let _e457: f32 = a_11;
                            let _e458: f32 = h_7;
                            wHaze = (_e455 + dot(_e456, vec3(((_e457 * _e458) / 3f))));
                            let _e465: vec3<f32> = (*trans_2);
                            let _e467: f32 = a_11;
                            (*trans_2) = (_e465 * (1f - _e467));
                        }
                    }
                }
            }
        }
    }
    let _e470: i32 = wEntry;
    if ((_e470 & ENTRY_FLAT) != 0i) {
        {
            alpha = clamp(u.knobs.y, 0f, 1f);
            let _e481: f32 = alpha;
            skinAlpha = min((_e481 * u.waterK.x), 0.95f);
            let _e492: bool = topRow;
            if _e492 {
                {
                    let _e493: f32 = skinAlpha;
                    a_12 = (_e493 * u.waterK.y);
                    c_28 = u.waterSurfaceC.xyz;
                    let _e501: i32 = z_83;
                    let _e503: i32 = RISE;
                    let _e506: i32 = row_11;
                    let _e507: i32 = skinRow;
                    let _e508: i32 = RISE;
                    let _e513: i32 = RISE;
                    let _e517: f32 = hazeAt((f32(_e501) + (f32(((_e503 - 1i) - (_e506 - (_e507 - _e508)))) / f32(_e513))));
                    hf = _e517;
                }
            } else {
                let _e518: bool = bodyRow;
                if _e518 {
                    {
                        let _e519: i32 = row_11;
                        let _e520: i32 = skinRow;
                        let _e522: bool = openUp_2;
                        isSkin = ((_e519 == _e520) && _e522);
                        let _e525: bool = isSkin;
                        if _e525 {
                            let _e526: f32 = skinAlpha;
                            local_76 = _e526;
                        } else {
                            let _e527: f32 = alpha;
                            local_76 = _e527;
                        }
                        let _e529: f32 = local_76;
                        a_12 = _e529;
                        let _e530: bool = isSkin;
                        if _e530 {
                            local_77 = u.waterSurfaceC.xyz;
                        } else {
                            local_77 = u.waterDeepC.xyz;
                        }
                        let _e538: vec3<f32> = local_77;
                        c_28 = _e538;
                        let _e539: i32 = z_83;
                        let _e541: f32 = hazeAt(f32(_e539));
                        hf = _e541;
                    }
                } else {
                    {
                        return;
                    }
                }
            }
            let _e542: vec3<f32> = wScatter;
            let _e543: vec3<f32> = (*trans_2);
            let _e544: f32 = a_12;
            let _e547: f32 = hf;
            let _e550: vec3<f32> = c_28;
            wScatter = (_e542 + (((_e543 * _e544) * (1f - _e547)) * _e550));
            let _e553: f32 = wHaze;
            let _e554: vec3<f32> = (*trans_2);
            let _e555: f32 = a_12;
            let _e556: f32 = hf;
            wHaze = (_e553 + dot(_e554, vec3(((_e555 * _e556) / 3f))));
            let _e563: vec3<f32> = (*trans_2);
            let _e565: f32 = a_12;
            (*trans_2) = (_e563 * (1f - _e565));
            return;
        }
    }
    let _e568: i32 = y_73;
    let _e570: i32 = fill_1;
    let _e572: i32 = S;
    top_11 = (f32(_e568) + (f32(_e570) / f32(_e572)));
    let _e577: f32 = yf_2;
    let _e578: f32 = top_11;
    hi_4 = min(_e577, _e578);
    let _e581: f32 = yf_2;
    let _e582: f32 = k_7;
    let _e584: i32 = y_73;
    lo_5 = max((_e581 - _e582), f32(_e584));
    let _e588: f32 = hi_4;
    let _e589: f32 = lo_5;
    let _e593: f32 = k_7;
    let _e596: f32 = k_7;
    let _e597: f32 = k_7;
    l_5 = ((max((_e588 - _e589), 0f) / _e593) * sqrt((1f + (_e596 * _e597))));
    let _e603: f32 = l_5;
    if (_e603 <= 0f) {
        {
            return;
        }
    }
    let _e606: vec3<f32> = waterSigma();
    let _e608: f32 = l_5;
    T = exp((-(_e606) * _e608));
    let _e612: f32 = wSurf;
    let _e614: f32 = hi_4;
    let _e615: f32 = lo_5;
    let _e622: f32 = k_7;
    let _e623: f32 = k_7;
    let _e628: f32 = k_7;
    depth = ((max((_e612 - (0.5f * (_e614 + _e615))), 0f) * sqrt((1f + (_e622 * _e623)))) / _e628);
    let _e642: f32 = depth;
    cin = mix(u.waterSurfaceC.xyz, u.waterDeepC.xyz, vec3((1f - exp((-(u.waterL.x) * _e642)))));
    let _e649: vec3<f32> = wScatter;
    let _e650: vec3<f32> = (*trans_2);
    let _e652: vec3<f32> = T;
    let _e657: f32 = h_7;
    let _e660: vec3<f32> = cin;
    wScatter = (_e649 + (((_e650 * (vec3(1f) - _e652)) * (1f - _e657)) * _e660));
    let _e663: f32 = wHaze;
    let _e664: vec3<f32> = (*trans_2);
    let _e666: vec3<f32> = T;
    let _e670: f32 = h_7;
    wHaze = (_e663 + dot((_e664 * (vec3(1f) - _e666)), vec3((_e670 / 3f))));
    let _e676: vec3<f32> = (*trans_2);
    let _e677: vec3<f32> = T;
    (*trans_2) = (_e676 * _e677);
    return;
}

fn waterAt(x_84: i32, y_74: i32, z_84: i32, v_60: vec4<u32>, row_12: i32, acc_1: ptr<function, vec3<f32>>, trans_3: ptr<function, vec3<f32>>) {
    var x_85: i32;
    var y_75: i32;
    var z_85: i32;
    var v_61: vec4<u32>;
    var row_13: i32;
    var fill_2: i32;
    var R_2: i32;
    var bottom_1: i32;
    var skinRow_1: i32;
    var alpha_1: f32;
    var skinAlpha_1: f32;
    var openUp_3: bool;
    var dy_29: i32;
    var c_29: vec3<f32>;
    var a_13: f32;
    var nearSolid_1: bool;
    var local_78: i32;
    var nearFill_1: i32;
    var nearSkin_1: i32;
    var stop_1: i32;
    var nearOpenUp_1: bool;
    var local_79: i32;
    var isSkin_1: bool;
    var haze_4: f32;
    var local_80: vec3<f32>;
    var c_30: vec3<f32>;
    var local_81: f32;
    var a_14: f32;

    x_85 = x_84;
    y_75 = y_74;
    z_85 = z_84;
    v_61 = v_60;
    row_13 = row_12;
    if LIT {
        {
            let _e200: i32 = x_85;
            let _e201: i32 = y_75;
            let _e202: i32 = z_85;
            let _e203: vec4<u32> = v_61;
            let _e204: i32 = row_13;
            waterAtLit(_e200, _e201, _e202, _e203, _e204, acc_1, trans_3);
            return;
        }
    }
    let _e209: vec4<u32> = v_61;
    let _e212: i32 = fillPxQ(i32(_e209.y));
    fill_2 = _e212;
    let _e214: i32 = y_75;
    let _e215: i32 = z_85;
    let _e216: i32 = frontRow(_e214, _e215);
    R_2 = _e216;
    let _e218: i32 = R_2;
    let _e219: i32 = S;
    bottom_1 = (_e218 + _e219);
    let _e222: i32 = bottom_1;
    let _e223: i32 = fill_2;
    skinRow_1 = (_e222 - _e223);
    alpha_1 = clamp(u.knobs.y, 0f, 1f);
    let _e233: f32 = alpha_1;
    skinAlpha_1 = min((_e233 * u.waterK.x), 0.95f);
    let _e241: i32 = x_85;
    let _e242: i32 = y_75;
    let _e243: i32 = z_85;
    let _e244: bool = waterOpenUp(_e241, _e242, _e243);
    openUp_3 = _e244;
    let _e246: bool = openUp_3;
    let _e247: i32 = row_13;
    let _e248: i32 = skinRow_1;
    let _e249: i32 = RISE;
    let _e253: i32 = row_13;
    let _e254: i32 = skinRow_1;
    let _e257: i32 = x_85;
    let _e258: i32 = y_75;
    let _e259: i32 = z_85;
    let _e260: i32 = row_13;
    let _e261: bool = nearerOwns(_e257, _e258, _e259, _e260);
    if (((_e246 && (_e247 >= (_e248 - _e249))) && (_e253 < _e254)) && !(_e261)) {
        {
            let _e264: i32 = row_13;
            let _e265: i32 = skinRow_1;
            let _e266: i32 = RISE;
            dy_29 = (_e264 - (_e265 - _e266));
            let _e273: i32 = z_85;
            let _e275: i32 = RISE;
            let _e278: i32 = dy_29;
            let _e281: i32 = RISE;
            let _e285: f32 = hazeAt((f32(_e273) + (f32(((_e275 - 1i) - _e278)) / f32(_e281))));
            let _e286: vec3<f32> = hazed(u.waterSurfaceC.xyz, _e285);
            c_29 = _e286;
            let _e288: f32 = skinAlpha_1;
            a_13 = (_e288 * u.waterK.y);
            let _e294: vec3<f32> = (*acc_1);
            let _e295: vec3<f32> = (*trans_3);
            let _e296: f32 = a_13;
            let _e298: vec3<f32> = c_29;
            (*acc_1) = (_e294 + ((_e295 * _e296) * _e298));
            let _e301: vec3<f32> = (*trans_3);
            let _e303: f32 = a_13;
            (*trans_3) = (_e301 * (1f - _e303));
        }
    }
    let _e306: i32 = z_85;
    let _e309: i32 = x_85;
    let _e310: i32 = y_75;
    let _e311: i32 = z_85;
    let _e314: bool = solidAt(_e309, _e310, (_e311 - 1i));
    nearSolid_1 = ((_e306 > 0i) && _e314);
    let _e317: i32 = z_85;
    if (_e317 > 0i) {
        let _e320: i32 = x_85;
        let _e321: i32 = y_75;
        let _e322: i32 = z_85;
        let _e325: i32 = fillPxAt(_e320, _e321, (_e322 - 1i));
        local_78 = _e325;
    } else {
        local_78 = 0i;
    }
    let _e328: i32 = local_78;
    nearFill_1 = _e328;
    let _e330: i32 = R_2;
    let _e331: i32 = RISE;
    let _e333: i32 = S;
    let _e335: i32 = nearFill_1;
    nearSkin_1 = (((_e330 + _e331) + _e333) - _e335);
    let _e338: i32 = bottom_1;
    stop_1 = _e338;
    let _e340: bool = nearSolid_1;
    if _e340 {
        {
            let _e341: i32 = skinRow_1;
            stop_1 = _e341;
        }
    } else {
        let _e342: i32 = nearFill_1;
        if (_e342 > 0i) {
            {
                let _e345: i32 = x_85;
                let _e346: i32 = y_75;
                let _e347: i32 = z_85;
                let _e350: bool = waterOpenUp(_e345, _e346, (_e347 - 1i));
                nearOpenUp_1 = _e350;
                let _e352: i32 = bottom_1;
                let _e353: bool = nearOpenUp_1;
                if _e353 {
                    let _e354: i32 = nearSkin_1;
                    let _e355: i32 = RISE;
                    local_79 = (_e354 - _e355);
                } else {
                    let _e357: i32 = nearSkin_1;
                    local_79 = _e357;
                }
                let _e359: i32 = local_79;
                stop_1 = min(_e352, _e359);
            }
        }
    }
    let _e361: i32 = row_13;
    let _e362: i32 = skinRow_1;
    let _e364: i32 = row_13;
    let _e365: i32 = stop_1;
    if ((_e361 >= _e362) && (_e364 < _e365)) {
        {
            let _e368: i32 = row_13;
            let _e369: i32 = skinRow_1;
            let _e371: bool = openUp_3;
            isSkin_1 = ((_e368 == _e369) && _e371);
            let _e374: i32 = z_85;
            let _e376: f32 = hazeAt(f32(_e374));
            haze_4 = _e376;
            let _e378: bool = isSkin_1;
            if _e378 {
                local_80 = u.waterSurfaceC.xyz;
            } else {
                local_80 = u.waterDeepC.xyz;
            }
            let _e386: vec3<f32> = local_80;
            let _e387: f32 = haze_4;
            let _e388: vec3<f32> = hazed(_e386, _e387);
            c_30 = _e388;
            let _e390: bool = isSkin_1;
            if _e390 {
                let _e391: f32 = skinAlpha_1;
                local_81 = _e391;
            } else {
                let _e392: f32 = alpha_1;
                local_81 = _e392;
            }
            let _e394: f32 = local_81;
            a_14 = _e394;
            let _e396: vec3<f32> = (*acc_1);
            let _e397: vec3<f32> = (*trans_3);
            let _e398: f32 = a_14;
            let _e400: vec3<f32> = c_30;
            (*acc_1) = (_e396 + ((_e397 * _e398) * _e400));
            let _e403: vec3<f32> = (*trans_3);
            let _e405: f32 = a_14;
            (*trans_3) = (_e403 * (1f - _e405));
            return;
        }
    } else {
        return;
    }
}

fn bandAt(t_23: f32) -> vec3<f32> {
    var t_24: f32;
    var b_8: f32;
    var c_31: vec3<f32>;

    t_24 = t_23;
    let _e192: f32 = t_24;
    b_8 = smoothstep(0.04f, 0.56f, _e192);
    let _e203: f32 = b_8;
    c_31 = mix(u.bandA.xyz, u.bandB.xyz, vec3(smoothstep(0f, 0.5f, _e203)));
    let _e208: vec3<f32> = c_31;
    let _e214: f32 = b_8;
    c_31 = mix(_e208, u.bandC.xyz, vec3(smoothstep(0.5f, 1f, _e214)));
    let _e218: vec3<f32> = c_31;
    let _e219: f32 = b_8;
    let _e220: f32 = b_8;
    return (_e218 * ((_e219 * _e220) * u.dayK.w));
}

fn skyAt(py: i32) -> vec3<f32> {
    var py_1: i32;
    var t_25: f32;
    var c_32: vec3<f32>;

    py_1 = py;
    if (u.roofK.w > 0.5f) {
        {
            let _e195: i32 = py_1;
            t_25 = clamp((f32((_e195 + u.camera.x)) / f32(max(u.camera.y, 1i))), 0f, 1f);
            let _e218: f32 = t_25;
            c_32 = mix(u.skyC.xyz, u.skyHorizon.xyz, vec3(_e218));
            if (u.dayK.w > 0f) {
                {
                    let _e227: vec3<f32> = c_32;
                    let _e228: f32 = t_25;
                    let _e229: vec3<f32> = bandAt(_e228);
                    c_32 = (_e227 + _e229);
                }
            }
            let _e231: vec3<f32> = c_32;
            return _e231;
        }
    }
    return u.skyC.xyz;
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
            let _e192: i32 = px_x_1;
            let _e193: i32 = px_y_1;
            let _e196: vec4<f32> = textureLoad(wgpu_backdropTex, vec2<i32>(_e192, _e193), 0i);
            return _e196.xyz;
        }
    }
    let _e198: i32 = px_y_1;
    let _e199: vec3<f32> = skyAt(_e198);
    sky = _e199;
    moisture = clamp(u.knobs.z, 0f, 1f);
    mistRows = min((u.extent.w / 3i), 56i);
    let _e221: f32 = moisture;
    let _e225: i32 = px_y_1;
    let _e226: i32 = mistRows;
    if (((u.roofK.w > 0.5f) && (_e221 > 0f)) && (_e225 < _e226)) {
        {
            let _e230: i32 = px_y_1;
            let _e232: i32 = mistRows;
            verticalT = (1f - (f32(_e230) / f32(_e232)));
            let _e237: f32 = verticalT;
            let _e238: f32 = verticalT;
            let _e240: f32 = moisture;
            rowFactor = ((_e237 * _e238) * _e240);
            driftX = (i32(u.knobs.w) / 2i);
            let _e250: i32 = px_x_1;
            let _e251: i32 = driftX;
            let _e253: i32 = px_y_1;
            wave1_ = ((sin((f32(((_e250 + _e251) + (_e253 * 4i))) * 0.045f)) * 0.5f) + 0.5f);
            let _e266: i32 = px_x_1;
            let _e269: i32 = driftX;
            wave2_ = ((cos((f32((((_e266 * 2i) - _e269) + 37i)) * 0.025f)) * 0.5f) + 0.5f);
            let _e282: f32 = wave1_;
            let _e283: f32 = wave2_;
            let _e285: f32 = rowFactor;
            density = ((_e282 * _e283) * _e285);
            let _e288: f32 = density;
            if (_e288 > 0.1f) {
                {
                    let _e291: f32 = density;
                    a_15 = min(((_e291 - 0.1f) * 1.8f), 0.7f);
                    let _e309: vec3<f32> = cloudC;
                    let _e310: vec3<f32> = cloudEdge;
                    let _e311: f32 = wave1_;
                    col_1 = mix(_e309, _e310, vec3((_e311 * 0.5f)));
                    let _e317: vec3<f32> = sky;
                    let _e318: vec3<f32> = col_1;
                    let _e319: f32 = a_15;
                    sky = mix(_e317, _e318, vec3(_e319));
                }
            }
        }
    }
    let _e322: vec3<f32> = sky;
    return _e322;
}

fn surfaceNear(x_86: i32, y_76: i32, z_86: i32) -> f32 {
    var x_87: i32;
    var y_77: i32;
    var z_87: i32;
    var local_82: i32;
    var ga: i32;
    var g_1: i32;
    var yy_1: i32;
    var gg: i32;

    x_87 = x_86;
    y_77 = y_76;
    z_87 = z_86;
    let _e197: i32 = z_87;
    let _e200: i32 = z_87;
    let _e201: i32 = D;
    let _e204: i32 = x_87;
    let _e205: i32 = y_77;
    let _e206: i32 = z_87;
    let _e207: bool = solidAt(_e204, _e205, _e206);
    if (((_e197 < 0i) || (_e200 >= _e201)) || _e207) {
        {
            return WALL;
        }
    }
    let _e209: i32 = x_87;
    let _e210: i32 = y_77;
    let _e213: i32 = z_87;
    let _e214: bool = solidAt(_e209, (_e210 + 1i), _e213);
    if _e214 {
        local_82 = 0i;
    } else {
        let _e216: i32 = x_87;
        let _e217: i32 = y_77;
        let _e220: i32 = z_87;
        let _e221: i32 = freeQ(_e216, (_e217 + 1i), _e220);
        local_82 = _e221;
    }
    let _e223: i32 = local_82;
    ga = _e223;
    let _e225: i32 = ga;
    if (_e225 > 0i) {
        {
            let _e228: i32 = y_77;
            let _e232: i32 = ga;
            return (f32((_e228 + 1i)) + (f32(_e232) / 255f));
        }
    }
    let _e237: i32 = x_87;
    let _e238: i32 = y_77;
    let _e239: i32 = z_87;
    let _e240: i32 = freeQ(_e237, _e238, _e239);
    g_1 = _e240;
    let _e242: i32 = g_1;
    if (_e242 > 0i) {
        {
            let _e245: i32 = y_77;
            let _e247: i32 = g_1;
            return (f32(_e245) + (f32(_e247) / 255f));
        }
    }
    let _e252: i32 = y_77;
    yy_1 = (_e252 - 1i);
    loop {
        let _e256: i32 = yy_1;
        let _e257: i32 = y_77;
        if !((_e256 >= max((_e257 - 2i), 0i))) {
            break;
        }
        {
            let _e267: i32 = x_87;
            let _e268: i32 = yy_1;
            let _e269: i32 = z_87;
            let _e270: bool = solidAt(_e267, _e268, _e269);
            if _e270 {
                {
                    let _e271: i32 = yy_1;
                    return f32((_e271 + 1i));
                }
            }
            let _e275: i32 = x_87;
            let _e276: i32 = yy_1;
            let _e277: i32 = z_87;
            let _e278: i32 = freeQ(_e275, _e276, _e277);
            gg = _e278;
            let _e280: i32 = gg;
            if (_e280 > 0i) {
                {
                    let _e283: i32 = yy_1;
                    let _e285: i32 = gg;
                    return (f32(_e283) + (f32(_e285) / 255f));
                }
            }
        }
        continuing {
            let _e264: i32 = yy_1;
            yy_1 = (_e264 - 1i);
        }
    }
    let _e290: i32 = y_77;
    return f32(max((_e290 - 2i), 0i));
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
    let _e197: f32 = a_17;
    wa = (_e197 < -500000f);
    let _e203: f32 = b_10;
    wb = (_e203 < -500000f);
    let _e209: bool = wa;
    let _e210: bool = wb;
    if (_e209 && _e210) {
        {
            return 0f;
        }
    }
    let _e213: bool = wa;
    if _e213 {
        {
            let _e214: f32 = b_10;
            let _e215: f32 = h0_1;
            return (_e214 - _e215);
        }
    }
    let _e217: bool = wb;
    if _e217 {
        {
            let _e218: f32 = h0_1;
            let _e219: f32 = a_17;
            return (_e218 - _e219);
        }
    }
    let _e222: f32 = b_10;
    let _e223: f32 = a_17;
    return (0.5f * (_e222 - _e223));
}

fn flowAt(c_33: vec3<i32>) -> vec3<f32> {
    var c_34: vec3<i32>;
    var x_88: i32;
    var y_78: i32;
    var z_88: i32;
    var h0_2: f32;
    var gx: f32;
    var gz: f32;
    var f_2: vec2<f32>;
    var m_13: f32;
    var q_10: f32;
    var local_83: vec2<f32>;

    c_34 = c_33;
    let _e193: vec3<i32> = c_34;
    x_88 = _e193.x;
    let _e196: vec3<i32> = c_34;
    y_78 = _e196.y;
    let _e199: vec3<i32> = c_34;
    z_88 = _e199.z;
    let _e202: i32 = y_78;
    let _e205: i32 = x_88;
    let _e206: i32 = y_78;
    let _e209: i32 = z_88;
    let _e210: bool = solidAt(_e205, (_e206 - 1i), _e209);
    let _e213: i32 = x_88;
    let _e214: i32 = y_78;
    let _e217: i32 = z_88;
    let _e218: i32 = fillPxAt(_e213, (_e214 - 1i), _e217);
    let _e219: i32 = S;
    if (((_e202 > 0i) && !(_e210)) && (_e218 < _e219)) {
        {
            return vec3<f32>(0f, 0f, 1f);
        }
    }
    let _e226: i32 = x_88;
    let _e227: i32 = y_78;
    let _e228: i32 = z_88;
    let _e229: f32 = surfaceNear(_e226, _e227, _e228);
    h0_2 = _e229;
    let _e231: i32 = x_88;
    let _e234: i32 = y_78;
    let _e235: i32 = z_88;
    let _e236: f32 = surfaceNear((_e231 - 1i), _e234, _e235);
    let _e237: f32 = h0_2;
    let _e238: i32 = x_88;
    let _e241: i32 = y_78;
    let _e242: i32 = z_88;
    let _e243: f32 = surfaceNear((_e238 + 1i), _e241, _e242);
    let _e244: f32 = slope1_(_e236, _e237, _e243);
    gx = _e244;
    let _e246: i32 = x_88;
    let _e247: i32 = y_78;
    let _e248: i32 = z_88;
    let _e251: f32 = surfaceNear(_e246, _e247, (_e248 - 1i));
    let _e252: f32 = h0_2;
    let _e253: i32 = x_88;
    let _e254: i32 = y_78;
    let _e255: i32 = z_88;
    let _e258: f32 = surfaceNear(_e253, _e254, (_e255 + 1i));
    let _e259: f32 = slope1_(_e251, _e252, _e258);
    gz = _e259;
    let _e261: f32 = gx;
    let _e262: f32 = gz;
    f_2 = -(vec2<f32>(_e261, _e262));
    let _e266: vec2<f32> = f_2;
    m_13 = length(_e266);
    let _e269: f32 = m_13;
    q_10 = (floor(((clamp((_e269 / FLOW_FULL), 0f, 1f) * FLOW_STEPS) + 0.5f)) / FLOW_STEPS);
    let _e280: f32 = m_13;
    if (_e280 > 0f) {
        let _e283: vec2<f32> = f_2;
        let _e284: f32 = m_13;
        let _e287: f32 = q_10;
        local_83 = ((_e283 / vec2(_e284)) * _e287);
    } else {
        local_83 = vec2(0f);
    }
    let _e292: vec2<f32> = local_83;
    return vec3<f32>(_e292.x, _e292.y, 0f);
}

fn wrapI(a_18: i32, m_14: i32) -> i32 {
    var a_19: i32;
    var m_15: i32;

    a_19 = a_18;
    m_15 = m_14;
    let _e195: i32 = a_19;
    let _e196: i32 = m_15;
    let _e197: i32 = wrapMod(_e195, _e196);
    return _e197;
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
    var h_8: u32;
    var hl: u32;
    var life: f32;
    var age: f32;
    var fade: f32;
    var f_3: f32;
    var mid: f32;
    var halfLen: f32;
    var along: f32;
    var taper: f32;
    var local_84: f32;

    w_6 = w_5;
    flow_1 = flow;
    phase_1 = phase;
    cycle_1 = cycle;
    salt_9 = salt_8;
    let _e209: f32 = phase_1;
    t_26 = ((_e209 * FLOW_CYCLE) * FLOW_ADVECT);
    let _e213: vec2<f32> = w_6;
    let _e214: vec2<f32> = flow_1;
    let _e215: f32 = t_26;
    a_20 = (_e213 - (_e214 * _e215));
    let _e219: vec2<f32> = a_20;
    let _e221: i32 = RISE;
    row_14 = i32(floor((_e219.y * f32(_e221))));
    let _e227: i32 = row_14;
    let _e229: i32 = cycle_1;
    let _e230: i32 = salt_9;
    let _e231: u32 = cellHash(_e227, 0i, _e229, _e230);
    hr = _e231;
    let _e234: vec2<f32> = a_20;
    let _e236: u32 = hr;
    let _e246: f32 = t_26;
    a_20.x = (_e234.x - (mix(BREEZE_MIN, BREEZE_MAX, (f32(((_e236 >> 8u) & 255u)) / 255f)) * _e246));
    let _e250: i32 = W;
    segs = max(1i, i32(((f32(_e250) / LINE_SEG) + 0.5f)));
    let _e258: vec2<f32> = a_20;
    let _e260: i32 = segs;
    let _e263: i32 = W;
    let _e266: u32 = hr;
    sx = (((_e258.x * f32(_e260)) / f32(_e263)) + (f32((_e266 & 255u)) / 256f));
    let _e274: f32 = sx;
    seg = i32(floor(_e274));
    let _e278: i32 = seg;
    let _e279: i32 = segs;
    let _e280: i32 = wrapI(_e278, _e279);
    let _e281: i32 = row_14;
    let _e282: i32 = cycle_1;
    let _e283: i32 = salt_9;
    let _e286: u32 = cellHash(_e280, _e281, _e282, (_e283 + 1i));
    h_8 = _e286;
    let _e288: u32 = h_8;
    if ((f32((_e288 & 65535u)) / 65535f) >= LINE_DENSITY) {
        {
            return vec2(0f);
        }
    }
    let _e297: i32 = seg;
    let _e298: i32 = segs;
    let _e299: i32 = wrapI(_e297, _e298);
    let _e300: i32 = row_14;
    let _e301: i32 = cycle_1;
    let _e302: i32 = salt_9;
    let _e305: u32 = cellHash(_e299, _e300, _e301, (_e302 + 2i));
    hl = _e305;
    let _e307: u32 = hl;
    life = mix(LINE_LIFE_MIN, LINE_LIFE_MAX, (f32((_e307 & 255u)) / 255f));
    let _e315: f32 = phase_1;
    let _e316: u32 = hl;
    let _e326: f32 = life;
    let _e330: f32 = life;
    age = ((_e315 - ((f32(((_e316 >> 8u) & 255u)) / 255f) * (1f - _e326))) / _e330);
    let _e333: f32 = age;
    let _e336: f32 = age;
    if ((_e333 <= 0f) || (_e336 >= 1f)) {
        {
            return vec2(0f);
        }
    }
    let _e343: f32 = age;
    fade = sin((3.1415927f * _e343));
    let _e347: f32 = fade;
    let _e348: f32 = fade;
    fade = (_e347 * _e348);
    let _e350: f32 = sx;
    let _e351: i32 = seg;
    f_3 = (_e350 - f32(_e351));
    let _e356: u32 = h_8;
    mid = (0.25f + ((f32(((_e356 >> 16u) & 255u)) / 255f) * 0.5f));
    let _e370: u32 = h_8;
    let _e382: f32 = fade;
    halfLen = ((0.1f + ((f32(((_e370 >> 24u) & 127u)) / 127f) * 0.15f)) * sqrt(_e382));
    let _e386: f32 = f_3;
    let _e387: f32 = mid;
    let _e390: f32 = halfLen;
    if (abs((_e386 - _e387)) >= _e390) {
        {
            return vec2(0f);
        }
    }
    let _e394: f32 = f_3;
    let _e395: f32 = mid;
    let _e397: f32 = halfLen;
    along = ((_e394 - _e395) / _e397);
    let _e401: f32 = along;
    let _e402: f32 = along;
    taper = (1f - (_e401 * _e402));
    let _e406: u32 = h_8;
    if ((_e406 & 2147483648u) != 0u) {
        local_84 = 1f;
    } else {
        local_84 = -1f;
    }
    let _e415: f32 = local_84;
    let _e416: f32 = fade;
    let _e418: f32 = fade;
    let _e419: f32 = taper;
    let _e421: f32 = taper;
    return vec2<f32>((_e415 * _e416), ((_e418 * _e419) * _e421));
}

fn rippleNormal(w_7: vec2<f32>, flow_2: vec2<f32>, glint: ptr<function, f32>) -> vec3<f32> {
    var w_8: vec2<f32>;
    var flow_3: vec2<f32>;
    var st: f32;
    var c1_: f32;
    var c2_: f32;
    var s_1: vec2<f32>;
    var n_4: vec3<f32>;

    w_8 = w_7;
    flow_3 = flow_2;
    st = u.clock.y;
    let _e208: f32 = st;
    c1_ = (_e208 / FLOW_CYCLE);
    let _e211: f32 = c1_;
    c2_ = (_e211 + 0.5f);
    let _e215: vec2<f32> = w_8;
    let _e216: vec2<f32> = flow_3;
    let _e217: f32 = c1_;
    let _e219: f32 = c1_;
    let _e223: i32 = wrapI(i32(floor(_e219)), 32i);
    let _e225: vec2<f32> = rippleLine(_e215, _e216, fract(_e217), _e223, 60i);
    s_1 = _e225;
    let _e227: vec2<f32> = s_1;
    if (_e227.x == 0f) {
        {
            let _e231: vec2<f32> = w_8;
            let _e236: vec2<f32> = flow_3;
            let _e237: f32 = c2_;
            let _e239: f32 = c2_;
            let _e243: i32 = wrapI(i32(floor(_e239)), 32i);
            let _e245: vec2<f32> = rippleLine((_e231 + vec2<f32>(0.37f, 0f)), _e236, fract(_e237), _e243, 62i);
            s_1 = _e245;
        }
    }
    (*glint) = 0f;
    let _e247: vec2<f32> = s_1;
    if (_e247.x == 0f) {
        {
            return vec3<f32>(0f, 1f, 0f);
        }
    }
    let _e257: vec2<f32> = s_1;
    n_4 = normalize(vec3<f32>(0f, 1f, (-(_e257.x) * u.waterL.z)));
    let _e267: vec3<f32> = n_4;
    if ((_e267.z * u.sunK.z) > 0f) {
        {
            let _e275: vec2<f32> = s_1;
            (*glint) = _e275.y;
        }
    }
    let _e277: vec3<f32> = n_4;
    return _e277;
}

fn hash01_(x_89: i32, y_79: i32, z_89: i32, salt_10: i32) -> f32 {
    var x_90: i32;
    var y_80: i32;
    var z_90: i32;
    var salt_11: i32;

    x_90 = x_89;
    y_80 = y_79;
    z_90 = z_89;
    salt_11 = salt_10;
    let _e207: i32 = x_90;
    let _e208: i32 = y_80;
    let _e209: i32 = z_90;
    let _e210: i32 = salt_11;
    let _e211: u32 = cellHash(_e207, _e208, _e209, _e210);
    return (f32((_e211 & 65535u)) / 65535f);
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
    var c_35: f32;
    var d: f32;

    w_10 = w_9;
    freq_1 = freq;
    layer_1 = layer;
    salt_13 = salt_12;
    let _e208: i32 = W;
    let _e210: f32 = freq_1;
    period = max(1i, i32(((f32(_e208) * _e210) + 0.5f)));
    let _e217: vec2<f32> = w_10;
    let _e219: i32 = period;
    let _e222: i32 = W;
    let _e225: vec2<f32> = w_10;
    let _e227: f32 = freq_1;
    q_11 = vec2<f32>(((_e217.x * f32(_e219)) / f32(_e222)), (_e225.y * _e227));
    let _e231: vec2<f32> = q_11;
    i_10 = floor(_e231);
    let _e234: vec2<f32> = q_11;
    let _e235: vec2<f32> = i_10;
    f_4 = (_e234 - _e235);
    let _e238: vec2<f32> = f_4;
    let _e239: vec2<f32> = f_4;
    let _e243: vec2<f32> = f_4;
    f_4 = ((_e238 * _e239) * (vec2(3f) - (2f * _e243)));
    let _e248: vec2<f32> = i_10;
    let _e251: i32 = period;
    let _e252: i32 = wrapI(i32(_e248.x), _e251);
    x0_ = _e252;
    let _e254: vec2<f32> = i_10;
    let _e259: i32 = period;
    let _e260: i32 = wrapI((i32(_e254.x) + 1i), _e259);
    x1_ = _e260;
    let _e262: vec2<f32> = i_10;
    y0_ = i32(_e262.y);
    let _e266: i32 = x0_;
    let _e267: i32 = y0_;
    let _e268: i32 = layer_1;
    let _e269: i32 = salt_13;
    let _e270: f32 = hash01_(_e266, _e267, _e268, _e269);
    a_21 = _e270;
    let _e272: i32 = x1_;
    let _e273: i32 = y0_;
    let _e274: i32 = layer_1;
    let _e275: i32 = salt_13;
    let _e276: f32 = hash01_(_e272, _e273, _e274, _e275);
    b_11 = _e276;
    let _e278: i32 = x0_;
    let _e279: i32 = y0_;
    let _e282: i32 = layer_1;
    let _e283: i32 = salt_13;
    let _e284: f32 = hash01_(_e278, (_e279 + 1i), _e282, _e283);
    c_35 = _e284;
    let _e286: i32 = x1_;
    let _e287: i32 = y0_;
    let _e290: i32 = layer_1;
    let _e291: i32 = salt_13;
    let _e292: f32 = hash01_(_e286, (_e287 + 1i), _e290, _e291);
    d = _e292;
    let _e294: f32 = a_21;
    let _e295: f32 = b_11;
    let _e296: vec2<f32> = f_4;
    let _e299: f32 = c_35;
    let _e300: f32 = d;
    let _e301: vec2<f32> = f_4;
    let _e304: vec2<f32> = f_4;
    return mix(mix(_e294, _e295, _e296.x), mix(_e299, _e300, _e301.x), _e304.y);
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
    let _e207: f32 = t_28;
    lt = floor(_e207);
    let _e210: f32 = t_28;
    let _e211: f32 = lt;
    ft = (_e210 - _e211);
    let _e214: f32 = ft;
    let _e215: f32 = ft;
    let _e219: f32 = ft;
    ft = ((_e214 * _e215) * (3f - (2f * _e219)));
    let _e223: f32 = lt;
    let _e226: i32 = wrapI(i32(_e223), 256i);
    l0_ = _e226;
    let _e228: f32 = lt;
    let _e233: i32 = wrapI((i32(_e228) + 1i), 256i);
    l1_ = _e233;
    let _e235: vec2<f32> = w_12;
    let _e236: f32 = freq_3;
    let _e237: i32 = l0_;
    let _e238: i32 = salt_15;
    let _e239: f32 = vnoise(_e235, _e236, _e237, _e238);
    let _e240: vec2<f32> = w_12;
    let _e241: f32 = freq_3;
    let _e242: i32 = l1_;
    let _e243: i32 = salt_15;
    let _e244: f32 = vnoise(_e240, _e241, _e242, _e243);
    let _e245: f32 = ft;
    return mix(_e239, _e244, _e245);
}

fn starAt(px: i32, py_2: i32, t_29: f32) -> vec4<f32> {
    var px_1: i32;
    var py_3: i32;
    var t_30: f32;
    var C: i32;
    var cell: vec2<i32>;
    var h_9: u32;
    var g_2: u32;
    var f_5: u32;
    var r_19: f32;
    var b_12: f32;
    var local_85: i32;
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
    var depth_1: f32;
    var tw: f32;
    var temp: f32;
    var local_86: vec3<f32>;
    var c_36: vec3<f32>;
    var rare: u32;
    var lum: f32;

    px_1 = px;
    py_3 = py_2;
    t_30 = t_29;
    let _e213: i32 = S;
    C = max(4i, i32(((f32(_e213) * STAR_CELL) + 0.5f)));
    let _e221: i32 = px_1;
    let _e222: i32 = C;
    let _e224: i32 = py_3;
    let _e225: i32 = C;
    cell = vec2<i32>((_e221 / _e222), (_e224 / _e225));
    let _e229: vec2<i32> = cell;
    let _e231: vec2<i32> = cell;
    let _e235: u32 = cellHash(_e229.x, _e231.y, 0i, 71i);
    h_9 = _e235;
    let _e237: u32 = h_9;
    if ((f32((_e237 & 65535u)) / 65536f) >= STAR_SHARE) {
        {
            return vec4(0f);
        }
    }
    let _e246: vec2<i32> = cell;
    let _e248: vec2<i32> = cell;
    let _e252: u32 = cellHash(_e246.x, _e248.y, 1i, 72i);
    g_2 = _e252;
    let _e254: vec2<i32> = cell;
    let _e256: vec2<i32> = cell;
    let _e260: u32 = cellHash(_e254.x, _e256.y, 2i, 73i);
    f_5 = _e260;
    let _e262: u32 = f_5;
    r_19 = ((f32((_e262 & 65535u)) + 1f) / 65536f);
    let _e271: f32 = r_19;
    b_12 = min((STAR_MIN * pow(_e271, -0.6666667f)), STAR_MAX);
    let _e280: i32 = S;
    if (_e280 >= 10i) {
        local_85 = 2i;
    } else {
        local_85 = 1i;
    }
    let _e286: i32 = local_85;
    sz = _e286;
    let _e288: i32 = C;
    let _e289: i32 = sz;
    span_1 = max(((_e288 - _e289) - 1i), 1i);
    let _e297: u32 = g_2;
    let _e298: i32 = span_1;
    let _e304: u32 = g_2;
    let _e308: i32 = span_1;
    at_1 = vec2<i32>((1i + i32((_e297 % u32(_e298)))), (1i + i32(((_e304 >> 8u) % u32(_e308)))));
    let _e315: i32 = px_1;
    let _e316: i32 = py_3;
    let _e318: vec2<i32> = cell;
    let _e319: i32 = C;
    let _e322: vec2<i32> = at_1;
    d_1 = ((vec2<i32>(_e315, _e316) - (_e318 * _e319)) - _e322);
    let _e326: vec2<i32> = d_1;
    let _e330: vec2<i32> = d_1;
    let _e335: vec2<i32> = d_1;
    let _e337: i32 = sz;
    let _e340: vec2<i32> = d_1;
    let _e342: i32 = sz;
    if ((((_e326.x >= 0i) && (_e330.y >= 0i)) && (_e335.x < _e337)) && (_e340.y < _e342)) {
        {
            k_8 = 1f;
        }
    } else {
        let _e346: f32 = b_12;
        let _e348: vec2<i32> = d_1;
        let _e353: vec2<i32> = d_1;
        let _e355: i32 = sz;
        let _e358: vec2<i32> = d_1;
        let _e363: vec2<i32> = d_1;
        let _e365: i32 = sz;
        let _e368: vec2<i32> = d_1;
        let _e373: vec2<i32> = d_1;
        let _e375: i32 = sz;
        let _e378: vec2<i32> = d_1;
        let _e383: vec2<i32> = d_1;
        let _e385: i32 = sz;
        if ((_e346 > STAR_BRIGHT) && (((((_e348.x == -1i) || (_e353.x == _e355)) && (_e358.y >= 0i)) && (_e363.y < _e365)) || ((((_e368.y == -1i) || (_e373.y == _e375)) && (_e378.x >= 0i)) && (_e383.x < _e385)))) {
            {
                let _e391: f32 = b_12;
                k_8 = (0.22f * min(((_e391 - STAR_BRIGHT) / STAR_BRIGHT), 1f));
            }
        } else {
            {
                return vec4(0f);
            }
        }
    }
    let _e401: u32 = g_2;
    w1_ = ((6.2831855f * f32((720u + ((_e401 >> 16u) % 2520u)))) / 3600f);
    let _e415: u32 = f_5;
    w2_ = ((6.2831855f * f32((250u + ((_e415 >> 16u) % 830u)))) / 3600f);
    let _e427: u32 = h_9;
    ph1_ = (f32((_e427 >> 16u)) * 0.0000958738f);
    let _e435: u32 = g_2;
    ph2_ = (f32((_e435 >> 24u)) * 0.024543693f);
    let _e449: f32 = w1_;
    let _e451: f32 = ph1_;
    let _e459: f32 = w2_;
    let _e461: f32 = ph2_;
    wave = (0.5f + (0.5f * ((0.65f * sin(((u.cloudK.w * _e449) + _e451))) + (0.35f * sin(((u.cloudK.w * _e459) + _e461))))));
    let _e470: f32 = b_12;
    faint = (1f - smoothstep(STAR_MIN, STAR_BRIGHT, _e470));
    let _e476: f32 = faint;
    let _e482: f32 = t_30;
    depth_1 = clamp((mix(0.18f, 0.55f, _e476) * (1f + (1.2f * smoothstep(0.1f, 0.5f, _e482)))), 0f, 0.9f);
    let _e492: f32 = depth_1;
    let _e493: f32 = wave;
    tw = (1f - (_e492 * _e493));
    let _e497: u32 = f_5;
    temp = (f32(((_e497 >> 16u) & 255u)) / 255f);
    let _e507: f32 = temp;
    if (_e507 < 0.55f) {
        let _e516: f32 = temp;
        local_86 = mix(vec3<f32>(0.72f, 0.84f, 1f), vec3(1f), vec3((_e516 / 0.55f)));
    } else {
        let _e527: f32 = temp;
        local_86 = mix(vec3(1f), vec3<f32>(1f, 0.82f, 0.62f), vec3(((_e527 - 0.55f) / 0.45f)));
    }
    let _e535: vec3<f32> = local_86;
    c_36 = _e535;
    let _e537: u32 = f_5;
    rare = ((_e537 >> 24u) % 16u);
    let _e544: u32 = rare;
    if (_e544 == 0u) {
        {
            let _e547: vec3<f32> = c_36;
            c_36 = mix(_e547, (u.lightC.xyz / vec3(max(max(u.lightC.x, u.lightC.y), max(u.lightC.z, 0.001f)))), vec3(0.5f));
        }
    }
    let _e569: u32 = rare;
    if (_e569 == 1u) {
        {
            let _e572: vec3<f32> = c_36;
            c_36 = mix(_e572, (u.bandB.xyz / vec3(max(max(u.bandB.x, u.bandB.y), max(u.bandB.z, 0.001f)))), vec3(0.4f));
        }
    }
    let _e594: f32 = b_12;
    let _e595: f32 = tw;
    let _e597: f32 = k_8;
    lum = ((_e594 * _e595) * _e597);
    let _e600: vec3<f32> = c_36;
    let _e601: f32 = lum;
    let _e602: vec3<f32> = (_e600 * _e601);
    let _e603: f32 = k_8;
    let _e604: f32 = b_12;
    return vec4<f32>(_e602.x, _e602.y, _e602.z, ((_e603 * max((_e604 - STAR_BRIGHT), 0f)) / 2.1f));
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
        let _e224: i32 = i_11;
        let _e225: i32 = octaves_1;
        if !((_e224 < _e225)) {
            break;
        }
        {
            let _e231: f32 = sum_2;
            let _e232: f32 = amp_6;
            let _e233: vec2<f32> = q_13;
            let _e234: f32 = freq_4;
            let _e235: i32 = i_11;
            let _e236: i32 = salt_17;
            let _e237: f32 = vnoise(_e233, _e234, _e235, _e236);
            sum_2 = (_e231 + (_e232 * _e237));
            let _e240: f32 = norm;
            let _e241: f32 = amp_6;
            norm = (_e240 + _e241);
            let _e243: f32 = amp_6;
            amp_6 = (_e243 * 0.5f);
            let _e246: f32 = freq_4;
            freq_4 = (_e246 * 2f);
        }
        continuing {
            let _e228: i32 = i_11;
            i_11 = (_e228 + 1i);
        }
    }
    let _e249: f32 = sum_2;
    let _e250: f32 = norm;
    return (_e249 / _e250);
}

fn cloudNoise(q_14: vec2<f32>) -> f32 {
    var q_15: vec2<f32>;
    var wq: vec2<f32>;
    var warp: vec2<f32>;

    q_15 = q_14;
    let _e211: vec2<f32> = q_15;
    wq = (_e211 - vec2<f32>((0.5f * u.cloudK.y), 0f));
    let _e221: vec2<f32> = wq;
    let _e224: f32 = cloudFbm(_e221, 3i, 120i);
    let _e225: vec2<f32> = wq;
    let _e232: f32 = cloudFbm((_e225 + vec2<f32>(64f, 23f)), 3i, 121i);
    warp = (vec2<f32>(_e224, _e232) - vec2(0.5f));
    let _e238: vec2<f32> = q_15;
    let _e239: vec2<f32> = warp;
    let _e244: f32 = cloudFbm((_e238 + (_e239 * CLOUD_WARP)), 5i, 122i);
    return _e244;
}

fn daySkyAt(t_31: f32) -> vec3<f32> {
    var t_32: f32;
    var g_3: f32;
    var c_37: vec3<f32>;

    t_32 = t_31;
    let _e213: f32 = t_32;
    g_3 = smoothstep(0f, DAY_HORIZON_T, _e213);
    let _e224: f32 = g_3;
    c_37 = mix(u.dayA.xyz, u.dayB.xyz, vec3(smoothstep(0f, 0.55f, _e224)));
    let _e229: vec3<f32> = c_37;
    let _e235: f32 = g_3;
    return mix(_e229, u.dayC.xyz, vec3(smoothstep(0.45f, 1f, _e235)));
}

fn litSky(px_2: i32, py_4: i32) -> vec4<f32> {
    var px_3: i32;
    var py_5: i32;
    var t_33: f32;
    var gradient: bool;
    var local_87: vec3<f32>;
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
    py_5 = py_4;
    let _e214: i32 = py_5;
    py_5 = (_e214 + u.camera.x);
    let _e219: i32 = py_5;
    t_33 = clamp((f32(_e219) / f32(max(u.camera.y, 1i))), 0f, 1f);
    gradient = (u.roofK.w > 0.5f);
    let _e238: bool = gradient;
    if _e238 {
        let _e245: f32 = t_33;
        local_87 = mix(u.skyC.xyz, u.skyHorizon.xyz, vec3(_e245));
    } else {
        local_87 = u.skyC.xyz;
    }
    let _e252: vec3<f32> = local_87;
    sky_1 = _e252;
    day = u.dayA.w;
    let _e258: f32 = day;
    if (_e258 > 0f) {
        {
            let _e263: f32 = t_33;
            b_13 = smoothstep(0.04f, 0.56f, _e263);
            let _e274: f32 = b_13;
            let _e283: f32 = b_13;
            warm = mix(mix(u.bandA.xyz, u.bandB.xyz, vec3(smoothstep(0f, 0.5f, _e274))), u.bandC.xyz, vec3(smoothstep(0.5f, 1f, _e283)));
            let _e288: vec3<f32> = sky_1;
            let _e289: vec3<f32> = warm;
            let _e290: f32 = t_33;
            let _e291: vec3<f32> = daySkyAt(_e290);
            let _e294: f32 = day;
            let _e298: f32 = day;
            sky_1 = mix(_e288, mix(_e289, _e291, vec3(smoothstep(0f, 1f, _e294))), vec3(_e298));
        }
    }
    let _e301: bool = gradient;
    if (_e301 && (u.dayK.w > 0f)) {
        {
            let _e308: i32 = px_3;
            let _e310: i32 = W;
            let _e311: i32 = S;
            xs = (((f32(_e308) / f32(max((_e310 * _e311), 1i))) * 2f) - 1f);
            let _e322: vec3<f32> = sky_1;
            let _e323: f32 = t_33;
            let _e324: vec3<f32> = bandAt(_e323);
            let _e331: f32 = xs;
            sky_1 = (_e322 + (_e324 * (1f + ((0.4f * u.sunLean.w) * _e331))));
        }
    }
    if (u.cloudBody.w > 0.001f) {
        {
            let _e343: i32 = px_3;
            let _e344: i32 = py_5;
            let _e345: f32 = t_33;
            let _e346: vec4<f32> = starAt(_e343, _e344, _e345);
            let _e353: f32 = t_33;
            st_1 = (_e346 * (u.cloudBody.w * (1f - smoothstep(0.25f, 0.6f, _e353))));
            let _e359: vec3<f32> = sky_1;
            let _e360: vec4<f32> = st_1;
            sky_1 = (_e359 + (_e360.xyz * STAR_GAIN));
            let _e364: vec4<f32> = st_1;
            glow_2 = (_e364.w * u.dayB.w);
        }
    }
    flash = u.flashC.w;
    let _e376: f32 = flash;
    if (_e376 > 0f) {
        {
            let _e379: i32 = W;
            let _e380: i32 = S;
            ringPx = f32((_e379 * _e380));
            let _e384: i32 = px_3;
            let _e391: i32 = S;
            dx_42 = abs(((f32(_e384) + 0.5f) - (u.strikeK.x * f32(_e391))));
            let _e397: f32 = dx_42;
            let _e398: f32 = ringPx;
            let _e399: f32 = dx_42;
            let _e403: i32 = S;
            dx_42 = (min(_e397, (_e398 - _e399)) / (28f * f32(_e403)));
            let _e407: f32 = dx_42;
            let _e409: f32 = dx_42;
            nearStrike = exp((-(_e407) * _e409));
            let _e412: vec3<f32> = sky_1;
            let _e418: f32 = nearStrike;
            sky_1 = (_e412 + (u.flashC.xyz * (0.04f + (0.06f * _e418))));
        }
    }
    cover = u.cloudK.x;
    let _e427: f32 = cover;
    if (_e427 < 0.001f) {
        {
            let _e430: vec3<f32> = sky_1;
            let _e431: f32 = glow_2;
            return vec4<f32>(_e430.x, _e430.y, _e430.z, _e431);
        }
    }
    storm = u.cloudK.z;
    let _e443: f32 = storm;
    let _e447: f32 = storm;
    let _e449: f32 = t_33;
    env = (1f - smoothstep(mix(0.22f, 0.5f, _e443), mix(0.52f, 0.8f, _e447), _e449));
    let _e453: f32 = env;
    if (_e453 <= 0f) {
        {
            let _e456: vec3<f32> = sky_1;
            let _e457: f32 = glow_2;
            return vec4<f32>(_e456.x, _e456.y, _e456.z, _e457);
        }
    }
    let _e462: i32 = px_3;
    let _e466: i32 = S;
    let _e473: i32 = py_5;
    let _e477: i32 = S;
    q_16 = vec2<f32>((((f32(_e462) + 0.5f) / f32(_e466)) + u.cloudK.y), (((f32(_e473) + 0.5f) / f32(_e477)) * CLOUD_FLAT));
    let _e483: vec2<f32> = q_16;
    let _e484: f32 = cloudNoise(_e483);
    n_5 = _e484;
    let _e488: f32 = cover;
    let _e491: f32 = storm;
    thr = (mix(0.74f, 0.36f, _e488) - (0.16f * _e491));
    let _e495: f32 = thr;
    let _e496: f32 = thr;
    let _e499: f32 = n_5;
    let _e501: f32 = env;
    let _e505: f32 = cover;
    d_2 = ((smoothstep(_e495, (_e496 + 0.08f), _e499) * _e501) * smoothstep(0f, 0.2f, _e505));
    let _e509: f32 = d_2;
    if (_e509 <= 0f) {
        {
            let _e512: vec3<f32> = sky_1;
            let _e513: f32 = glow_2;
            return vec4<f32>(_e512.x, _e512.y, _e512.z, _e513);
        }
    }
    toSun = normalize(vec2<f32>(u.sunK.x, (-(max(u.sunK.y, 0.25f)) * CLOUD_FLAT)));
    let _e531: vec2<f32> = q_16;
    let _e532: vec2<f32> = toSun;
    let _e536: f32 = cloudNoise((_e531 + (_e532 * 2f)));
    n2_ = _e536;
    let _e538: f32 = n_5;
    let _e539: f32 = n2_;
    edge = clamp((((_e538 - _e539) * 10f) + 0.2f), 0f, 1f);
    let _e549: f32 = edge;
    lit_6 = (_e549 * u.cloudLit.w);
    let _e560: f32 = d_2;
    let _e563: f32 = lit_6;
    let _e570: f32 = storm;
    body_6 = ((u.cloudBody.xyz * (1.15f - ((0.45f * _e560) * (1f - _e563)))) * (1f - (0.5f * _e570)));
    let _e575: vec3<f32> = body_6;
    let _e579: f32 = lit_6;
    col_2 = (_e575 + (u.cloudLit.xyz * _e579));
    let _e583: f32 = day;
    if (_e583 > 0f) {
        {
            let _e586: f32 = edge;
            let _e592: f32 = cover;
            let _e594: f32 = cover;
            let _e602: f32 = storm;
            sunward = (clamp((((_e586 * 2f) + 0.3f) - ((0.4f * _e592) * _e594)), 0f, 1f) * (1f - (0.7f * _e602)));
            let _e613: f32 = sunward;
            let _e618: f32 = storm;
            dayCol = (mix(u.cloudShade.xyz, u.cloudDay.xyz, vec3(_e613)) * (1f - (0.72f * _e618)));
            let _e623: vec3<f32> = col_2;
            let _e624: vec3<f32> = dayCol;
            let _e625: f32 = day;
            col_2 = mix(_e623, _e624, vec3(_e625));
        }
    }
    let _e628: f32 = flash;
    if (_e628 > 0f) {
        {
            let _e631: vec3<f32> = col_2;
            let _e637: f32 = nearStrike;
            let _e643: f32 = d_2;
            col_2 = (_e631 + ((u.flashC.xyz * (0.12f + (0.5f * _e637))) * (0.4f + (0.6f * _e643))));
        }
    }
    let _e648: vec3<f32> = sky_1;
    let _e649: vec3<f32> = col_2;
    let _e650: f32 = d_2;
    let _e654: vec3<f32> = mix(_e648, _e649, vec3((_e650 * 0.92f)));
    let _e655: f32 = glow_2;
    let _e657: f32 = d_2;
    return vec4<f32>(_e654.x, _e654.y, _e654.z, (_e655 * (1f - _e657)));
}

fn shoreDist(c_38: vec3<i32>, p_21: vec3<f32>, brim: bool) -> f32 {
    var c_39: vec3<i32>;
    var p_22: vec3<f32>;
    var brim_1: bool;
    var fx_1: f32;
    var fz_1: f32;
    var d_3: f32 = 9f;
    var dz: i32 = -1i;
    var dx_43: i32;
    var nx: i32;
    var nz: i32;
    var local_88: f32;
    var local_89: f32;
    var ex: f32;
    var local_90: f32;
    var local_91: f32;
    var ez: f32;

    c_39 = c_38;
    p_22 = p_21;
    brim_1 = brim;
    let _e221: vec3<f32> = p_22;
    let _e223: vec3<i32> = c_39;
    fx_1 = (_e221.x - f32(_e223.x));
    let _e228: vec3<f32> = p_22;
    let _e230: vec3<i32> = c_39;
    fz_1 = (_e228.z - f32(_e230.z));
    loop {
        let _e240: i32 = dz;
        if !((_e240 <= 1i)) {
            break;
        }
        {
            dx_43 = -1i;
            loop {
                let _e250: i32 = dx_43;
                if !((_e250 <= 1i)) {
                    break;
                }
                {
                    let _e257: i32 = dx_43;
                    let _e260: i32 = dz;
                    if ((_e257 == 0i) && (_e260 == 0i)) {
                        {
                            continue;
                        }
                    }
                    let _e264: vec3<i32> = c_39;
                    let _e266: i32 = dx_43;
                    nx = (_e264.x + _e266);
                    let _e269: vec3<i32> = c_39;
                    let _e271: i32 = dz;
                    nz = (_e269.z + _e271);
                    let _e274: i32 = nz;
                    let _e277: i32 = nz;
                    let _e278: i32 = D;
                    if ((_e274 < 0i) || (_e277 >= _e278)) {
                        {
                            continue;
                        }
                    }
                    let _e281: i32 = nx;
                    let _e282: vec3<i32> = c_39;
                    let _e284: i32 = nz;
                    let _e285: bool = solidAt(_e281, _e282.y, _e284);
                    let _e287: bool = brim_1;
                    let _e288: i32 = nx;
                    let _e289: vec3<i32> = c_39;
                    let _e293: i32 = nz;
                    let _e294: bool = solidAt(_e288, (_e289.y + 1i), _e293);
                    if (!(_e285) && !((_e287 && _e294))) {
                        {
                            continue;
                        }
                    }
                    let _e298: i32 = dx_43;
                    if (_e298 < 0i) {
                        let _e301: f32 = fx_1;
                        local_89 = _e301;
                    } else {
                        let _e302: i32 = dx_43;
                        if (_e302 > 0i) {
                            let _e306: f32 = fx_1;
                            local_88 = (1f - _e306);
                        } else {
                            local_88 = 0f;
                        }
                        let _e310: f32 = local_88;
                        local_89 = _e310;
                    }
                    let _e312: f32 = local_89;
                    ex = _e312;
                    let _e314: i32 = dz;
                    if (_e314 < 0i) {
                        let _e317: f32 = fz_1;
                        local_91 = _e317;
                    } else {
                        let _e318: i32 = dz;
                        if (_e318 > 0i) {
                            let _e322: f32 = fz_1;
                            local_90 = (1f - _e322);
                        } else {
                            local_90 = 0f;
                        }
                        let _e326: f32 = local_90;
                        local_91 = _e326;
                    }
                    let _e328: f32 = local_91;
                    ez = _e328;
                    let _e330: f32 = d_3;
                    let _e331: f32 = ex;
                    let _e332: f32 = ez;
                    d_3 = min(_e330, length(vec2<f32>(_e331, _e332)));
                }
                continuing {
                    let _e254: i32 = dx_43;
                    dx_43 = (_e254 + 1i);
                }
            }
        }
        continuing {
            let _e244: i32 = dz;
            dz = (_e244 + 1i);
        }
    }
    let _e336: f32 = d_3;
    return _e336;
}

fn plungeDist(c_40: vec3<i32>, p_23: vec3<f32>, yp: i32) -> f32 {
    var c_41: vec3<i32>;
    var p_24: vec3<f32>;
    var yp_1: i32;
    var d_4: f32 = 9f;
    var dz_1: i32 = -1i;
    var dx_44: i32;
    var nx_1: i32;
    var nz_1: i32;

    c_41 = c_40;
    p_24 = p_23;
    yp_1 = yp;
    loop {
        let _e226: i32 = dz_1;
        if !((_e226 <= 1i)) {
            break;
        }
        {
            dx_44 = -1i;
            loop {
                let _e236: i32 = dx_44;
                if !((_e236 <= 1i)) {
                    break;
                }
                {
                    let _e243: vec3<i32> = c_41;
                    let _e245: i32 = dx_44;
                    nx_1 = (_e243.x + _e245);
                    let _e248: vec3<i32> = c_41;
                    let _e250: i32 = dz_1;
                    nz_1 = (_e248.z + _e250);
                    let _e253: i32 = nz_1;
                    let _e256: i32 = nz_1;
                    let _e257: i32 = D;
                    let _e260: i32 = nx_1;
                    let _e261: i32 = yp_1;
                    let _e262: i32 = nz_1;
                    let _e263: i32 = freeQ(_e260, _e261, _e262);
                    let _e267: i32 = nx_1;
                    let _e268: i32 = yp_1;
                    let _e269: i32 = nz_1;
                    let _e270: bool = fallingAt(_e267, _e268, _e269);
                    if ((((_e253 < 0i) || (_e256 >= _e257)) || (_e263 == 0i)) || !(_e270)) {
                        {
                            continue;
                        }
                    }
                    let _e273: f32 = d_4;
                    let _e274: vec3<f32> = p_24;
                    let _e276: i32 = nx_1;
                    let _e280: i32 = nz_1;
                    d_4 = min(_e273, length((_e274.xz - vec2<f32>((f32(_e276) + 0.5f), (f32(_e280) + 0.5f)))));
                }
                continuing {
                    let _e240: i32 = dx_44;
                    dx_44 = (_e240 + 1i);
                }
            }
        }
        continuing {
            let _e230: i32 = dz_1;
            dz_1 = (_e230 + 1i);
        }
    }
    let _e288: f32 = d_4;
    return _e288;
}

fn foamAt(c_42: vec3<i32>, p_25: vec3<f32>, fill_3: i32) -> f32 {
    var c_43: vec3<i32>;
    var p_26: vec3<f32>;
    var fill_4: i32;
    var brim_2: bool;
    var st_2: f32;
    var cover_1: f32 = 0f;
    var ds: f32;
    var t_34: f32;
    var w_13: f32;
    var lace: f32;
    var local_92: i32;
    var dp: f32;
    var churn: f32;

    c_43 = c_42;
    p_26 = p_25;
    fill_4 = fill_3;
    let _e221: i32 = fill_4;
    let _e222: i32 = S;
    brim_2 = (_e221 >= _e222);
    st_2 = u.clock.y;
    let _e231: vec3<i32> = c_43;
    let _e232: vec3<f32> = p_26;
    let _e233: bool = brim_2;
    let _e234: f32 = shoreDist(_e231, _e232, _e233);
    ds = _e234;
    let _e236: f32 = ds;
    if (_e236 < 0.58000004f) {
        {
            let _e243: f32 = st_2;
            t_34 = (_e243 / FOAM_STEPS);
            let _e246: vec3<f32> = p_26;
            let _e249: f32 = t_34;
            let _e251: f32 = vnoiseT(_e246.xz, 1f, _e249, 80i);
            w_13 = (FOAM_WIDTH * (FOAM_BREATHE + _e251));
            let _e257: vec3<f32> = p_26;
            let _e260: f32 = t_34;
            let _e262: f32 = vnoiseT(_e257.xz, 3f, _e260, 81i);
            lace = smoothstep(0.3f, 0.7f, _e262);
            let _e267: f32 = w_13;
            let _e269: f32 = w_13;
            let _e270: f32 = ds;
            let _e275: f32 = lace;
            cover_1 = ((1f - smoothstep((0.35f * _e267), _e269, _e270)) * mix(0.45f, 1f, _e275));
        }
    }
    let _e278: vec3<i32> = c_43;
    let _e279: vec3<f32> = p_26;
    let _e280: bool = brim_2;
    if _e280 {
        let _e281: vec3<i32> = c_43;
        local_92 = (_e281.y + 1i);
    } else {
        let _e285: vec3<i32> = c_43;
        local_92 = _e285.y;
    }
    let _e288: i32 = local_92;
    let _e291: f32 = plungeDist(_e278, _e279, (_e288 + 1i));
    dp = _e291;
    let _e293: f32 = dp;
    if (_e293 < PLUNGE_RADIUS) {
        {
            let _e295: vec3<f32> = p_26;
            let _e298: f32 = st_2;
            let _e301: f32 = vnoiseT(_e295.xz, 3f, (_e298 / PLUNGE_STEPS), 82i);
            churn = _e301;
            let _e303: f32 = cover_1;
            let _e306: f32 = dp;
            let _e311: f32 = churn;
            cover_1 = max(_e303, ((1f - smoothstep(0.25f, PLUNGE_RADIUS, _e306)) * mix(0.5f, 1f, _e311)));
        }
    }
    let _e315: f32 = cover_1;
    return _e315;
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
    var h_10: u32;
    var chance: f32;
    var hv: f32;
    var margin: f32;
    var age_1: f32;
    var m_16: vec2<f32>;
    var at_2: vec2<f32>;
    var centre: vec2<f32>;
    var e_7: vec2<f32>;
    var l_6: f32;
    var r_20: f32;
    var local_93: vec2<f32>;
    var grad: vec2<f32>;
    var px_4: f32;
    var edge_1: f32;
    var fade_1: f32;

    w_15 = w_14;
    copy_1 = copy;
    let _e228: i32 = copy_1;
    tt = ((u.clock.y / RAIN_SLOT) + (0.5f * f32(_e228)));
    let _e233: f32 = tt;
    let _e237: i32 = wrapI(i32(floor(_e233)), 256i);
    slot_9 = _e237;
    let _e239: f32 = tt;
    phase_2 = fract(_e239);
    let _e243: i32 = W;
    cellsX = max(1i, i32(((f32(_e243) / RAIN_CELL) + 0.5f)));
    let _e251: i32 = W;
    let _e253: i32 = cellsX;
    cellW = (f32(_e251) / f32(_e253));
    let _e257: vec2<f32> = w_15;
    let _e259: f32 = cellW;
    let _e261: vec2<f32> = w_15;
    let _e266: i32 = copy_1;
    g_4 = (vec2<f32>((_e257.x / _e259), (_e261.y / RAIN_CELL)) + vec2((0.5f * f32(_e266))));
    let _e272: vec2<f32> = g_4;
    cell_1 = vec2<i32>(floor(_e272));
    let _e276: vec2<i32> = cell_1;
    let _e278: i32 = cellsX;
    let _e279: i32 = wrapI(_e276.x, _e278);
    let _e280: vec2<i32> = cell_1;
    let _e282: i32 = slot_9;
    let _e284: i32 = copy_1;
    let _e286: u32 = cellHash(_e279, _e280.y, _e282, (90i + _e284));
    h_10 = _e286;
    chance = min(u.rainK.w, 0.98f);
    let _e294: u32 = h_10;
    hv = (f32((_e294 & 65535u)) / 65535f);
    let _e301: f32 = hv;
    let _e302: f32 = chance;
    if (_e301 >= _e302) {
        {
            return 0f;
        }
    }
    let _e305: f32 = chance;
    let _e306: f32 = hv;
    margin = clamp(((_e305 - _e306) / 0.06f), 0f, 1f);
    let _e314: f32 = phase_2;
    let _e315: u32 = h_10;
    age_1 = ((_e314 - ((f32(((_e315 >> 16u) & 255u)) / 255f) * 0.3f)) / RING_LIFE);
    let _e331: f32 = age_1;
    let _e334: f32 = age_1;
    if ((_e331 <= 0f) || (_e334 >= 1f)) {
        {
            return 0f;
        }
    }
    let _e339: f32 = cellW;
    m_16 = vec2<f32>((RING_RADIUS / _e339), 0.3f);
    let _e346: u32 = h_10;
    let _e353: u32 = h_10;
    at_2 = (vec2<f32>(f32(((_e346 >> 24u) & 15u)), f32(((_e353 >> 28u) & 15u))) / vec2(15f));
    let _e365: vec2<f32> = m_16;
    let _e366: vec2<f32> = at_2;
    let _e369: vec2<f32> = m_16;
    centre = (_e365 + (_e366 * (vec2(1f) - (2f * _e369))));
    let _e376: vec2<f32> = g_4;
    let _e377: vec2<f32> = g_4;
    let _e380: vec2<f32> = centre;
    let _e382: f32 = cellW;
    e_7 = (((_e376 - floor(_e377)) - _e380) * vec2<f32>(_e382, RAIN_CELL));
    let _e386: vec2<f32> = e_7;
    l_6 = length(_e386);
    let _e391: f32 = age_1;
    let _e394: f32 = age_1;
    r_20 = (RING_RADIUS * (1f - ((1f - _e391) * (1f - _e394))));
    let _e400: f32 = l_6;
    if (_e400 > 0.0001f) {
        let _e403: vec2<f32> = e_7;
        let _e404: f32 = l_6;
        let _e407: i32 = S;
        let _e409: i32 = RISE;
        local_93 = ((_e403 / vec2(_e404)) / vec2<f32>(f32(_e407), f32(_e409)));
    } else {
        let _e414: i32 = S;
        local_93 = vec2<f32>((1f / f32(_e414)), 0f);
    }
    let _e420: vec2<f32> = local_93;
    grad = _e420;
    let _e422: f32 = l_6;
    let _e423: f32 = r_20;
    let _e426: vec2<f32> = grad;
    px_4 = (abs((_e422 - _e423)) / max(length(_e426), 0.000001f));
    let _e433: f32 = px_4;
    edge_1 = max((1f - (_e433 / 0.8f)), 0f);
    let _e442: f32 = age_1;
    let _e445: f32 = age_1;
    let _e449: f32 = age_1;
    fade_1 = ((smoothstep(0f, 0.12f, _e442) * (1f - _e445)) * (1f - _e449));
    let _e453: f32 = edge_1;
    let _e454: f32 = fade_1;
    let _e456: f32 = margin;
    return ((_e453 * _e454) * _e456);
}

fn undersideLit(c_44: vec3<i32>, v_62: vec4<u32>, hp: vec3<f32>) -> vec3<f32> {
    var c_45: vec3<i32>;
    var v_63: vec4<u32>;
    var hp_1: vec3<f32>;
    var body_7: vec3<f32>;

    c_45 = c_44;
    v_63 = v_62;
    hp_1 = hp;
    let _e225: vec4<u32> = v_63;
    let _e226: vec4<u32> = v_63;
    let _e227: i32 = matOf(_e226);
    let _e228: vec3<f32> = strataOf(_e227);
    let _e229: vec3<f32> = blockBody(_e225, _e228);
    body_7 = _e229;
    let _e231: vec3<f32> = body_7;
    let _e234: vec3<f32> = hp_1;
    let _e240: vec3<f32> = glowAt((_e234 - vec3<f32>(0f, 0.5f, 0f)));
    let _e241: vec3<f32> = ladderLight(0f, 0f, _e240);
    let _e243: vec3<i32> = c_45;
    let _e246: f32 = hazeAt(f32(_e243.z));
    let _e247: vec3<f32> = hazed((_e231 * _e241), _e246);
    return _e247;
}

fn reflectTexel(hp_2: vec3<f32>, c_46: vec3<i32>, axis_1: i32, dirX: f32) -> vec2<i32> {
    var hp_3: vec3<f32>;
    var c_47: vec3<i32>;
    var axis_2: i32;
    var dirX_1: f32;
    var dx_45: i32;
    var dy_30: i32;
    var local_94: i32;

    hp_3 = hp_2;
    c_47 = c_46;
    axis_2 = axis_1;
    dirX_1 = dirX;
    let _e227: vec3<f32> = hp_3;
    let _e230: i32 = S;
    let _e236: i32 = S;
    dx_45 = clamp(i32(floor((fract(_e227.x) * f32(_e230)))), 0i, (_e236 - 1i));
    let _e241: vec3<i32> = c_47;
    let _e246: vec3<f32> = hp_3;
    let _e249: i32 = S;
    let _e255: i32 = S;
    dy_30 = clamp(i32(floor(((f32((_e241.y + 1i)) - _e246.y) * f32(_e249)))), 0i, (_e255 - 1i));
    let _e260: i32 = axis_2;
    if (_e260 == 0i) {
        {
            let _e263: f32 = dirX_1;
            if (_e263 > 0f) {
                local_94 = 0i;
            } else {
                let _e267: i32 = S;
                local_94 = (_e267 - 1i);
            }
            let _e271: i32 = local_94;
            dx_45 = _e271;
        }
    }
    let _e272: i32 = axis_2;
    if (_e272 == 1i) {
        {
            let _e275: i32 = S;
            dy_30 = (_e275 - 1i);
        }
    }
    let _e278: i32 = S;
    let _e281: i32 = dy_30;
    let _e283: i32 = dx_45;
    return vec2<i32>(((_e278 - 1i) - _e281), _e283);
}

fn glyphFilled(base_11: i32, col_3: i32, row_15: i32) -> bool {
    var base_12: i32;
    var col_4: i32;
    var row_16: i32;

    base_12 = base_11;
    col_4 = col_3;
    row_16 = row_15;
    let _e225: i32 = col_4;
    let _e228: i32 = col_4;
    let _e229: i32 = S;
    let _e232: i32 = row_16;
    let _e236: i32 = row_16;
    let _e237: i32 = S;
    if ((((_e225 < 0i) || (_e228 >= _e229)) || (_e232 < 0i)) || (_e236 >= _e237)) {
        {
            return true;
        }
    }
    let _e241: i32 = col_4;
    let _e242: i32 = base_12;
    let _e243: i32 = row_16;
    let _e247: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e241, (_e242 + _e243)), 0i);
    return ((_e247.x >> 2u) != 63u);
}

fn glyphTexelAt(v_64: vec4<u32>, hp_4: vec3<f32>, c_48: vec3<i32>, axis_3: i32, dirX_2: f32) -> bool {
    var v_65: vec4<u32>;
    var hp_5: vec3<f32>;
    var c_49: vec3<i32>;
    var axis_4: i32;
    var dirX_3: f32;
    var t_35: vec2<i32>;
    var base_13: i32;
    var row_17: i32;
    var col_5: i32;

    v_65 = v_64;
    hp_5 = hp_4;
    c_49 = c_48;
    axis_4 = axis_3;
    dirX_3 = dirX_2;
    let _e229: vec3<f32> = hp_5;
    let _e230: vec3<i32> = c_49;
    let _e231: i32 = axis_4;
    let _e232: f32 = dirX_3;
    let _e233: vec2<i32> = reflectTexel(_e229, _e230, _e231, _e232);
    t_35 = _e233;
    let _e235: vec4<u32> = v_65;
    let _e236: i32 = partOf(_e235);
    let _e239: vec4<u32> = v_65;
    let _e240: i32 = glyphOf(_e239);
    let _e242: i32 = S;
    let _e243: i32 = RISE;
    base_13 = (((_e236 * 8i) + _e240) * (_e242 + _e243));
    let _e247: i32 = S;
    let _e250: vec2<i32> = t_35;
    row_17 = ((_e247 - 1i) - _e250.x);
    let _e254: vec2<i32> = t_35;
    col_5 = _e254.y;
    let _e257: i32 = base_13;
    let _e258: i32 = col_5;
    let _e259: i32 = row_17;
    let _e260: bool = glyphFilled(_e257, _e258, _e259);
    let _e261: i32 = base_13;
    let _e262: i32 = col_5;
    let _e265: i32 = row_17;
    let _e266: bool = glyphFilled(_e261, (_e262 - 1i), _e265);
    let _e268: i32 = base_13;
    let _e269: i32 = col_5;
    let _e272: i32 = row_17;
    let _e273: bool = glyphFilled(_e268, (_e269 + 1i), _e272);
    let _e275: i32 = base_13;
    let _e276: i32 = col_5;
    let _e277: i32 = row_17;
    let _e280: bool = glyphFilled(_e275, _e276, (_e277 - 1i));
    let _e282: i32 = base_13;
    let _e283: i32 = col_5;
    let _e284: i32 = row_17;
    let _e287: bool = glyphFilled(_e282, _e283, (_e284 + 1i));
    return ((((_e260 && _e266) && _e273) && _e280) && _e287);
}

fn reflectMarch(p_27: vec3<f32>, r_21: vec3<f32>, hit: ptr<function, vec3<i32>>, hp_6: ptr<function, vec3<f32>>, axis_5: ptr<function, i32>) -> bool {
    var p_28: vec3<f32>;
    var r_22: vec3<f32>;
    var c_50: vec3<i32>;
    var dir_1: vec3<i32>;
    var local_95: f32;
    var local_96: f32;
    var local_97: f32;
    var inv_1: vec3<f32>;
    var local_98: f32;
    var local_99: f32;
    var local_100: f32;
    var next_1: vec3<f32>;
    var cap_2: i32;
    var tIn: f32 = 0f;
    var i_12: i32 = 0i;
    var v_66: vec4<u32>;
    var local_101: i32;
    var local_102: i32;

    p_28 = p_27;
    r_22 = r_21;
    let _e226: vec3<f32> = p_28;
    let _e227: vec3<f32> = r_22;
    c_50 = vec3<i32>(floor((_e226 + (_e227 * 0.001f))));
    let _e234: vec3<f32> = r_22;
    dir_1 = vec3<i32>(sign(_e234));
    let _e238: vec3<f32> = r_22;
    if (abs(_e238.x) > 0.000001f) {
        let _e244: vec3<f32> = r_22;
        local_95 = (1f / abs(_e244.x));
    } else {
        local_95 = 1000000000000000000000000000000f;
    }
    let _e250: f32 = local_95;
    let _e251: vec3<f32> = r_22;
    if (abs(_e251.y) > 0.000001f) {
        let _e257: vec3<f32> = r_22;
        local_96 = (1f / abs(_e257.y));
    } else {
        local_96 = 1000000000000000000000000000000f;
    }
    let _e263: f32 = local_96;
    let _e264: vec3<f32> = r_22;
    if (abs(_e264.z) > 0.000001f) {
        let _e270: vec3<f32> = r_22;
        local_97 = (1f / abs(_e270.z));
    } else {
        local_97 = 1000000000000000000000000000000f;
    }
    let _e276: f32 = local_97;
    inv_1 = vec3<f32>(_e250, _e263, _e276);
    let _e279: vec3<i32> = dir_1;
    if (_e279.x > 0i) {
        let _e283: vec3<i32> = c_50;
        let _e288: vec3<f32> = p_28;
        local_98 = (f32((_e283.x + 1i)) - _e288.x);
    } else {
        let _e291: vec3<f32> = p_28;
        let _e293: vec3<i32> = c_50;
        local_98 = (_e291.x - f32(_e293.x));
    }
    let _e298: f32 = local_98;
    let _e299: vec3<f32> = inv_1;
    let _e302: vec3<i32> = dir_1;
    if (_e302.y > 0i) {
        let _e306: vec3<i32> = c_50;
        let _e311: vec3<f32> = p_28;
        local_99 = (f32((_e306.y + 1i)) - _e311.y);
    } else {
        let _e314: vec3<f32> = p_28;
        let _e316: vec3<i32> = c_50;
        local_99 = (_e314.y - f32(_e316.y));
    }
    let _e321: f32 = local_99;
    let _e322: vec3<f32> = inv_1;
    let _e325: vec3<i32> = dir_1;
    if (_e325.z > 0i) {
        let _e329: vec3<i32> = c_50;
        let _e334: vec3<f32> = p_28;
        local_100 = (f32((_e329.z + 1i)) - _e334.z);
    } else {
        let _e337: vec3<f32> = p_28;
        let _e339: vec3<i32> = c_50;
        local_100 = (_e337.z - f32(_e339.z));
    }
    let _e344: f32 = local_100;
    let _e345: vec3<f32> = inv_1;
    next_1 = vec3<f32>((_e298 * _e299.x), (_e321 * _e322.y), (_e344 * _e345.z));
    cap_2 = i32(u.waterL.w);
    (*axis_5) = -1i;
    let _e359: vec3<i32> = c_50;
    (*hit) = _e359;
    loop {
        let _e362: i32 = i_12;
        let _e363: i32 = cap_2;
        if !((_e362 < _e363)) {
            break;
        }
        {
            let _e369: vec3<i32> = c_50;
            let _e371: i32 = H;
            let _e373: vec3<i32> = c_50;
            let _e378: vec3<i32> = c_50;
            let _e383: vec3<i32> = c_50;
            let _e385: i32 = D;
            if ((((_e369.y >= _e371) || (_e373.y < 0i)) || (_e378.z < 0i)) || (_e383.z >= _e385)) {
                {
                    break;
                }
            }
            let _e388: i32 = i_12;
            if (_e388 > 0i) {
                {
                    let _e391: vec3<i32> = c_50;
                    let _e393: vec3<i32> = c_50;
                    let _e395: vec3<i32> = c_50;
                    let _e397: vec4<u32> = at(_e391.x, _e393.y, _e395.z);
                    v_66 = _e397;
                    let _e399: vec4<u32> = v_66;
                    let _e400: bool = solidV(_e399);
                    let _e401: vec4<u32> = v_66;
                    let _e402: i32 = partOf(_e401);
                    let _e403: bool = isBlockPart(_e402);
                    let _e404: vec4<u32> = v_66;
                    let _e405: vec3<f32> = p_28;
                    let _e406: vec3<f32> = r_22;
                    let _e407: f32 = tIn;
                    let _e410: vec3<i32> = c_50;
                    let _e411: i32 = (*axis_5);
                    let _e412: vec3<f32> = r_22;
                    let _e414: bool = glyphTexelAt(_e404, (_e405 + (_e406 * _e407)), _e410, _e411, _e412.x);
                    if (_e400 || (_e403 && _e414)) {
                        {
                            let _e417: vec3<i32> = c_50;
                            (*hit) = _e417;
                            let _e418: vec3<f32> = p_28;
                            let _e419: vec3<f32> = r_22;
                            let _e420: f32 = tIn;
                            (*hp_6) = (_e418 + (_e419 * _e420));
                            return true;
                        }
                    }
                }
            }
            let _e424: vec3<f32> = next_1;
            let _e426: vec3<f32> = next_1;
            let _e429: vec3<f32> = next_1;
            let _e431: vec3<f32> = next_1;
            if ((_e424.x < _e426.y) && (_e429.x < _e431.z)) {
                local_102 = 0i;
            } else {
                let _e436: vec3<f32> = next_1;
                let _e438: vec3<f32> = next_1;
                if (_e436.y < _e438.z) {
                    local_101 = 1i;
                } else {
                    local_101 = 2i;
                }
                let _e444: i32 = local_101;
                local_102 = _e444;
            }
            let _e446: i32 = local_102;
            (*axis_5) = _e446;
            let _e447: i32 = (*axis_5);
            if (_e447 == 0i) {
                {
                    let _e450: vec3<f32> = next_1;
                    tIn = _e450.x;
                    let _e453: vec3<i32> = c_50;
                    let _e455: vec3<i32> = dir_1;
                    c_50.x = (_e453.x + _e455.x);
                    let _e459: vec3<f32> = next_1;
                    let _e461: vec3<f32> = inv_1;
                    next_1.x = (_e459.x + _e461.x);
                }
            } else {
                let _e464: i32 = (*axis_5);
                if (_e464 == 1i) {
                    {
                        let _e467: vec3<f32> = next_1;
                        tIn = _e467.y;
                        let _e470: vec3<i32> = c_50;
                        let _e472: vec3<i32> = dir_1;
                        c_50.y = (_e470.y + _e472.y);
                        let _e476: vec3<f32> = next_1;
                        let _e478: vec3<f32> = inv_1;
                        next_1.y = (_e476.y + _e478.y);
                    }
                } else {
                    {
                        let _e481: vec3<f32> = next_1;
                        tIn = _e481.z;
                        let _e484: vec3<i32> = c_50;
                        let _e486: vec3<i32> = dir_1;
                        c_50.z = (_e484.z + _e486.z);
                        let _e490: vec3<f32> = next_1;
                        let _e492: vec3<f32> = inv_1;
                        next_1.z = (_e490.z + _e492.z);
                    }
                }
            }
        }
        continuing {
            let _e366: i32 = i_12;
            i_12 = (_e366 + 1i);
        }
    }
    let _e495: vec3<f32> = p_28;
    let _e496: vec3<f32> = r_22;
    let _e497: f32 = tIn;
    (*hp_6) = (_e495 + (_e496 * _e497));
    return false;
}

fn reflectionShade(found: bool, c_51: vec3<i32>, hp_7: vec3<f32>, axis_6: i32, r_23: vec3<f32>) -> vec3<f32> {
    var found_1: bool;
    var c_52: vec3<i32>;
    var hp_8: vec3<f32>;
    var axis_7: i32;
    var r_24: vec3<f32>;
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
    var local_103: f32;

    found_1 = found;
    c_52 = c_51;
    hp_8 = hp_7;
    axis_7 = axis_6;
    r_24 = r_23;
    let _e229: bool = found_1;
    if !(_e229) {
        {
            let _e231: i32 = BASE;
            let _e233: vec3<f32> = hp_8;
            let _e235: i32 = S;
            let _e239: vec3<f32> = hp_8;
            let _e241: i32 = RISE;
            row_18 = i32(floor(((f32(_e231) - (_e233.y * f32(_e235))) - (_e239.z * f32(_e241)))));
            let _e248: vec3<f32> = hp_8;
            let _e250: i32 = S;
            let _e255: i32 = W;
            let _e256: i32 = S;
            let _e260: i32 = wrapI(i32(floor((_e248.x * f32(_e250)))), max((_e255 * _e256), 1i));
            col_6 = _e260;
            let _e262: i32 = col_6;
            let _e263: i32 = row_18;
            let _e273: vec4<f32> = textureLoad(wgpu_backdropTex, vec2<i32>(_e262, clamp(_e263, 0i, (u.extent.w - 1i))), 0i);
            return _e273.xyz;
        }
    }
    let _e275: vec3<i32> = c_52;
    let _e277: vec3<i32> = c_52;
    let _e279: vec3<i32> = c_52;
    let _e281: vec4<u32> = at(_e275.x, _e277.y, _e279.z);
    v_67 = _e281;
    let _e283: vec3<f32> = hp_8;
    let _e284: vec3<i32> = c_52;
    let _e285: i32 = axis_7;
    let _e286: vec3<f32> = r_24;
    let _e288: vec2<i32> = reflectTexel(_e283, _e284, _e285, _e286.x);
    t_36 = _e288;
    let _e290: i32 = S;
    let _e293: vec2<i32> = t_36;
    dy_31 = ((_e290 - 1i) - _e293.x);
    let _e297: vec4<u32> = v_67;
    let _e298: bool = solidV(_e297);
    solid = _e298;
    let _e300: bool = solid;
    let _e301: i32 = axis_7;
    if (_e300 && (_e301 == 1i)) {
        {
            let _e305: vec3<i32> = c_52;
            let _e306: vec4<u32> = v_67;
            let _e307: vec3<f32> = hp_8;
            let _e308: vec3<f32> = undersideLit(_e305, _e306, _e307);
            return _e308;
        }
    }
    let _e311: bool = solid;
    if _e311 {
        {
            let _e312: vec4<u32> = v_67;
            let _e313: i32 = matOf(_e312);
            m_17 = _e313;
            let _e315: vec4<u32> = v_67;
            let _e316: i32 = m_17;
            let _e318: i32 = m_17;
            let _e322: vec3<i32> = c_52;
            let _e324: vec3<i32> = c_52;
            let _e326: vec3<i32> = c_52;
            let _e329: vec2<i32> = t_36;
            let _e331: i32 = dy_31;
            let _e332: vec3<f32> = faceBase(_e316, (2i * (_e318 - 1i)), _e322.x, _e324.y, _e326.z, 0i, _e329.y, _e331);
            let _e333: vec3<f32> = blockBody(_e315, _e332);
            base_14 = _e333;
            let _e334: vec3<i32> = c_52;
            let _e336: vec3<i32> = c_52;
            let _e340: vec3<i32> = c_52;
            let _e342: f32 = canopyAt(_e334.x, (_e336.z - 1i), _e340.y);
            canopy_4 = _e342;
        }
    } else {
        {
            let _e343: vec4<u32> = v_67;
            let _e344: i32 = partOf(_e343);
            let _e347: vec4<u32> = v_67;
            let _e348: i32 = glyphOf(_e347);
            let _e350: i32 = S;
            let _e351: i32 = RISE;
            let _e354: i32 = dy_31;
            atlasY_1 = ((((_e344 * 8i) + _e348) * (_e350 + _e351)) + _e354);
            let _e357: vec2<i32> = t_36;
            let _e359: i32 = atlasY_1;
            let _e362: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e357.y, _e359), 0i);
            q_17 = _e362.x;
            let _e365: u32 = q_17;
            let _e369: vec4<u32> = v_67;
            let _e374: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(i32((_e365 & 3u)), i32(_e369.w)), 0i);
            pigment_2 = _e374;
            let _e376: vec4<u32> = v_67;
            let _e379: vec3<f32> = styleEmit(i32(_e376.w));
            e_8 = _e379;
            let _e381: u32 = q_17;
            let _e387: vec4<f32> = pigment_2;
            let _e389: bool = emitsWhole(_e387.w);
            emits = ((i32((_e381 >> 2u)) == TONE_EMIT) || _e389);
            let _e394: bool = emits;
            if _e394 {
                {
                    let _e395: vec3<i32> = c_52;
                    let _e397: vec3<i32> = c_52;
                    let _e399: vec3<i32> = c_52;
                    let _e405: bool = cellEmit(_e395.x, _e397.y, _e399.z, (&e_8), (&on_4));
                }
            }
            let _e406: bool = emits;
            let _e407: vec3<f32> = e_8;
            if (_e406 && any((_e407 > vec3(0f)))) {
                {
                    let _e413: vec3<f32> = e_8;
                    let _e414: vec3<i32> = c_52;
                    let _e417: f32 = hazeAt(f32(_e414.z));
                    let _e418: vec3<f32> = hazed(_e413, _e417);
                    return _e418;
                }
            }
            let _e419: vec4<f32> = pigment_2;
            base_14 = _e419.xyz;
            let _e421: vec3<i32> = c_52;
            let _e423: vec3<i32> = c_52;
            let _e425: vec3<i32> = c_52;
            let _e427: vec4<u32> = v_67;
            let _e429: vec2<i32> = t_36;
            let _e431: i32 = dy_31;
            let _e434: bool = plantTexel(_e421.x, _e423.y, _e425.z, _e427, false, _e429.y, _e431, (&base_14));
            if !(_e434) {
                {
                    let _e436: vec4<u32> = v_67;
                    let _e440: vec3<f32> = styleAt(i32(_e436.w), 1i);
                    base_14 = _e440;
                }
            }
            let _e441: vec4<u32> = v_67;
            let _e442: i32 = partOf(_e441);
            if (_e442 == ANIMAL_INTERIM) {
                let _e444: vec3<i32> = c_52;
                let _e446: vec3<i32> = c_52;
                let _e450: vec3<i32> = c_52;
                let _e452: f32 = canopyAt(_e444.x, (_e446.z - 1i), _e450.y);
                local_103 = _e452;
            } else {
                let _e453: vec4<u32> = v_67;
                let _e454: f32 = plantCanopy(_e453);
                local_103 = _e454;
            }
            let _e456: f32 = local_103;
            canopy_4 = _e456;
        }
    }
    let _e457: vec3<f32> = base_14;
    let _e458: vec3<i32> = c_52;
    let _e460: vec3<i32> = c_52;
    let _e462: vec3<i32> = c_52;
    let _e464: vec2<i32> = t_36;
    let _e466: i32 = dy_31;
    let _e467: f32 = canopy_4;
    let _e468: vec3<f32> = shadeFront(_e457, _e458.x, _e460.y, _e462.z, _e464.y, _e466, _e467);
    let _e469: vec3<i32> = c_52;
    let _e472: f32 = hazeAt(f32(_e469.z));
    let _e473: vec3<f32> = hazed(_e468, _e472);
    return _e473;
}

fn fallStreak(px_5: i32, py_6: i32, st_3: f32) -> bool {
    var px_6: i32;
    var py_7: i32;
    var st_4: f32;
    var h_11: u32;
    var local_104: i32;
    var period_1: i32;
    var len: i32;
    var speed: i32;
    var ph: i32;

    px_6 = px_5;
    py_7 = py_6;
    st_4 = st_3;
    let _e225: i32 = px_6;
    let _e226: i32 = W;
    let _e227: i32 = S;
    let _e229: i32 = wrapI(_e225, (_e226 * _e227));
    let _e233: u32 = cellHash(_e229, 0i, 0i, 50i);
    h_11 = _e233;
    let _e235: u32 = h_11;
    if ((_e235 % 3u) != 0u) {
        {
            return false;
        }
    }
    let _e241: i32 = S;
    if (_e241 >= 8i) {
        local_104 = 32i;
    } else {
        local_104 = 16i;
    }
    let _e247: i32 = local_104;
    period_1 = _e247;
    let _e250: i32 = S;
    let _e254: u32 = h_11;
    len = (max(2i, (_e250 / 3i)) + i32(((_e254 >> 8u) & 1u)));
    let _e264: i32 = S;
    speed = max(1i, (_e264 / 4i));
    let _e269: i32 = py_7;
    let _e270: f32 = st_4;
    let _e271: i32 = speed;
    let _e277: u32 = h_11;
    let _e283: i32 = period_1;
    let _e284: i32 = wrapI(((_e269 - i32(floor((_e270 * f32(_e271))))) + i32((_e277 >> 16u))), _e283);
    ph = _e284;
    let _e286: i32 = ph;
    let _e287: i32 = len;
    return (_e286 < _e287);
}

fn flowDebug(fl: vec3<f32>, top_12: bool, p_29: vec3<f32>) -> vec3<f32> {
    var fl_1: vec3<f32>;
    var top_13: bool;
    var p_30: vec3<f32>;
    var local_105: vec3<f32>;
    var q_18: f32;
    var a_22: f32;
    var hue_1: vec3<f32>;
    var c_53: vec3<f32>;
    var lp: vec2<f32>;
    var o_3: vec2<f32>;
    var d_5: vec2<f32>;
    var e_9: vec2<f32>;
    var pa: vec2<f32>;
    var ba: vec2<f32>;
    var t_37: f32;
    var local_106: vec3<f32>;

    fl_1 = fl;
    top_13 = top_12;
    p_30 = p_29;
    let _e225: vec3<f32> = fl_1;
    if (_e225.z > 0.5f) {
        {
            let _e229: i32 = PX;
            let _e230: i32 = PY;
            let _e234: bool = fallStreak(_e229, _e230, u.clock.y);
            if _e234 {
                local_105 = vec3(1f);
            } else {
                local_105 = vec3<f32>(0.35f, 0.05f, 0.5f);
            }
            let _e242: vec3<f32> = local_105;
            return _e242;
        }
    }
    let _e243: vec3<f32> = fl_1;
    q_18 = length(_e243.xy);
    let _e247: f32 = q_18;
    if (_e247 < 0.01f) {
        {
            return vec3<f32>(0.01f, 0.02f, 0.2f);
        }
    }
    let _e254: vec3<f32> = fl_1;
    let _e256: vec3<f32> = fl_1;
    a_22 = atan2(_e254.y, _e256.x);
    let _e262: f32 = a_22;
    let _e264: f32 = a_22;
    let _e268: f32 = a_22;
    hue_1 = (vec3(0.5f) + (0.5f * vec3<f32>(cos(_e262), cos((_e264 - 2.0944f)), cos((_e268 + 2.0944f)))));
    let _e277: vec3<f32> = hue_1;
    let _e280: f32 = q_18;
    c_53 = (_e277 * (0.25f + (0.75f * _e280)));
    let _e285: bool = top_13;
    let _e286: i32 = S;
    if (_e285 && (_e286 >= 8i)) {
        {
            let _e290: vec3<f32> = p_30;
            let _e293: i32 = S;
            let _e297: vec3<f32> = p_30;
            let _e301: i32 = RISE;
            lp = vec2<f32>((fract(_e290.x) * f32(_e293)), ((1f - fract(_e297.z)) * f32(_e301)));
            let _e307: i32 = S;
            let _e311: i32 = RISE;
            o_3 = vec2<f32>((0.5f * f32(_e307)), (0.5f * f32(_e311)));
            let _e316: vec3<f32> = fl_1;
            let _e318: i32 = S;
            let _e321: vec3<f32> = fl_1;
            let _e324: i32 = RISE;
            d_5 = normalize(vec2<f32>((_e316.x * f32(_e318)), (-(_e321.y) * f32(_e324))));
            let _e330: vec2<f32> = o_3;
            let _e331: vec2<f32> = d_5;
            let _e334: i32 = S;
            let _e338: i32 = RISE;
            let _e340: i32 = S;
            e_9 = (_e330 + (((_e331 * 0.45f) * f32(_e334)) * vec2<f32>(1f, (f32(_e338) / f32(_e340)))));
            let _e347: vec2<f32> = lp;
            let _e348: vec2<f32> = o_3;
            pa = (_e347 - _e348);
            let _e351: vec2<f32> = e_9;
            let _e352: vec2<f32> = o_3;
            ba = (_e351 - _e352);
            let _e355: vec2<f32> = pa;
            let _e356: vec2<f32> = ba;
            let _e358: vec2<f32> = ba;
            let _e359: vec2<f32> = ba;
            t_37 = clamp((dot(_e355, _e356) / max(dot(_e358, _e359), 0.0001f)), 0f, 1f);
            let _e368: vec2<f32> = pa;
            let _e369: vec2<f32> = ba;
            let _e370: f32 = t_37;
            if (length((_e368 - (_e369 * _e370))) < 0.75f) {
                {
                    let _e376: f32 = t_37;
                    if (_e376 > 0.6f) {
                        local_106 = vec3<f32>(1f, 1f, 0.3f);
                    } else {
                        local_106 = vec3(1f);
                    }
                    let _e386: vec3<f32> = local_106;
                    c_53 = _e386;
                }
            }
        }
    }
    let _e387: vec3<f32> = c_53;
    return _e387;
}

fn litWaterFinish(colour: vec3<f32>) -> vec3<f32> {
    var colour_1: vec3<f32>;
    var local_107: vec3<f32>;
    var zf_2: i32;
    var local_108: vec3<f32>;
    var y_81: i32;
    var z_91: i32;
    var top_14: bool;
    var x_91: i32;
    var fill_5: i32;
    var k_9: f32;
    var yf_3: f32;
    var xw: f32;
    var ys_1: f32;
    var local_109: vec3<f32>;
    var p_31: vec3<f32>;
    var local_110: i32;
    var local_111: vec3<i32>;
    var open: vec3<i32>;
    var local_112: f32;
    var canopy_5: f32;
    var fl_2: vec3<f32>;
    var local_113: f32;
    var sunOn_2: f32;
    var local_114: f32;
    var ndl_4: f32;
    var local_115: f32;
    var sun_6: f32;
    var local_116: vec3<f32>;
    var glow_3: vec3<f32>;
    var here: vec3<f32>;
    var ratio: vec3<f32>;
    var glint_1: f32;
    var n_6: vec3<f32>;
    var d_6: vec3<f32>;
    var cosi: f32;
    var f_6: f32;
    var r_25: vec3<f32>;
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
    let _e221: i32 = wEntry;
    if ((_e221 & ENTRY_FLAT) != 0i) {
        {
            let _e225: vec3<f32> = colour_1;
            let _e226: vec3<f32> = wScatter;
            let _e231: f32 = wHaze;
            colour_1 = (_e225 + ((_e226 * u.dayL.x) + (_e231 * u.hazeC.xyz)));
            let _e238: i32 = wEntry;
            if ((_e238 & ENTRY_FALLING) == 0i) {
                {
                    let _e242: vec3<f32> = colour_1;
                    return _e242;
                }
            }
            if (u.clock.z > 0.5f) {
                {
                    let _e248: i32 = PX;
                    let _e249: i32 = PY;
                    let _e253: bool = fallStreak(_e248, _e249, u.clock.y);
                    if _e253 {
                        local_107 = vec3(1f);
                    } else {
                        local_107 = vec3<f32>(0.35f, 0.05f, 0.5f);
                    }
                    let _e261: vec3<f32> = local_107;
                    return _e261;
                }
            }
            let _e262: i32 = wEntry;
            zf_2 = ((_e262 >> 12u) & 4095i);
            let _e269: i32 = PX;
            let _e270: i32 = PY;
            let _e274: bool = fallStreak(_e269, _e270, u.clock.y);
            if _e274 {
                let _e282: i32 = zf_2;
                let _e284: f32 = hazeAt(f32(_e282));
                let _e285: vec3<f32> = hazed((u.waterSurfaceC.xyz * u.dayL.x), _e284);
                local_108 = _e285;
            } else {
                let _e286: vec3<f32> = colour_1;
                local_108 = _e286;
            }
            let _e288: vec3<f32> = local_108;
            return _e288;
        }
    }
    let _e289: i32 = wEntry;
    y_81 = (_e289 & 4095i);
    let _e293: i32 = wEntry;
    z_91 = ((_e293 >> 12u) & 4095i);
    let _e300: i32 = wEntry;
    top_14 = ((_e300 >> 24u) != 0i);
    let _e307: i32 = PX;
    let _e308: i32 = S;
    x_91 = (_e307 / _e308);
    let _e311: i32 = x_91;
    let _e312: i32 = y_81;
    let _e313: i32 = z_91;
    let _e314: i32 = fillPxAt(_e311, _e312, _e313);
    fill_5 = _e314;
    let _e316: i32 = RISE;
    let _e318: i32 = S;
    k_9 = (f32(_e316) / f32(_e318));
    let _e322: i32 = BASE;
    let _e323: i32 = z_91;
    let _e324: i32 = RISE;
    let _e327: i32 = PY;
    let _e334: i32 = S;
    yf_3 = ((f32((((_e322 - (_e323 * _e324)) - _e327) - 1i)) + 0.5f) / f32(_e334));
    let _e338: i32 = PX;
    let _e342: i32 = S;
    xw = ((f32(_e338) + 0.5f) / f32(_e342));
    let _e346: i32 = y_81;
    let _e348: i32 = fill_5;
    let _e350: i32 = S;
    ys_1 = (f32(_e346) + (f32(_e348) / f32(_e350)));
    let _e355: bool = top_14;
    if _e355 {
        let _e356: f32 = xw;
        let _e357: f32 = ys_1;
        let _e358: i32 = z_91;
        let _e360: f32 = yf_3;
        let _e361: f32 = ys_1;
        let _e363: f32 = k_9;
        local_109 = vec3<f32>(_e356, _e357, (f32(_e358) + clamp(((_e360 - _e361) / _e363), 0f, 1f)));
    } else {
        let _e370: f32 = xw;
        let _e371: f32 = yf_3;
        let _e372: i32 = z_91;
        local_109 = vec3<f32>(_e370, _e371, f32(_e372));
    }
    let _e376: vec3<f32> = local_109;
    p_31 = _e376;
    let _e378: bool = top_14;
    if _e378 {
        let _e379: i32 = x_91;
        let _e380: i32 = fill_5;
        let _e381: i32 = S;
        if (_e380 < _e381) {
            let _e383: i32 = y_81;
            local_110 = _e383;
        } else {
            let _e384: i32 = y_81;
            local_110 = (_e384 + 1i);
        }
        let _e388: i32 = local_110;
        let _e389: i32 = z_91;
        local_111 = vec3<i32>(_e379, _e388, _e389);
    } else {
        let _e391: i32 = x_91;
        let _e392: i32 = y_81;
        let _e393: i32 = z_91;
        local_111 = vec3<i32>(_e391, _e392, (_e393 - 1i));
    }
    let _e398: vec3<i32> = local_111;
    open = _e398;
    let _e400: bool = top_14;
    if _e400 {
        let _e401: i32 = x_91;
        let _e402: i32 = z_91;
        let _e403: i32 = y_81;
        let _e404: f32 = canopyAt(_e401, _e402, _e403);
        local_112 = _e404;
    } else {
        let _e405: i32 = x_91;
        let _e406: i32 = z_91;
        let _e409: i32 = y_81;
        let _e410: f32 = canopyAt(_e405, (_e406 - 1i), _e409);
        local_112 = _e410;
    }
    let _e412: f32 = local_112;
    canopy_5 = _e412;
    let _e414: i32 = x_91;
    let _e415: i32 = y_81;
    let _e416: i32 = z_91;
    let _e418: vec3<f32> = flowAt(vec3<i32>(_e414, _e415, _e416));
    fl_2 = _e418;
    if (u.clock.z > 0.5f) {
        {
            let _e425: vec3<f32> = fl_2;
            let _e426: bool = top_14;
            let _e427: vec3<f32> = p_31;
            let _e428: vec3<f32> = flowDebug(_e425, _e426, _e427);
            return _e428;
        }
    }
    if (u.sunK.y > 0f) {
        local_113 = 1f;
    } else {
        local_113 = 0f;
    }
    let _e437: f32 = local_113;
    sunOn_2 = _e437;
    let _e439: bool = top_14;
    if _e439 {
        local_114 = u.sunK.y;
    } else {
        local_114 = -(u.sunK.z);
    }
    let _e448: f32 = local_114;
    ndl_4 = _e448;
    let _e450: f32 = sunOn_2;
    let _e453: f32 = ndl_4;
    if ((_e450 > 0f) && (_e453 > 0f)) {
        let _e457: vec3<f32> = p_31;
        let _e458: vec3<i32> = open;
        let _e459: f32 = sunReaches(_e457, _e458);
        local_115 = _e459;
    } else {
        local_115 = 0f;
    }
    let _e462: f32 = local_115;
    sun_6 = _e462;
    let _e464: bool = top_14;
    if _e464 {
        let _e465: vec3<f32> = p_31;
        local_116 = (_e465 + vec3<f32>(0f, 0.5f, 0f));
    } else {
        let _e471: vec3<f32> = p_31;
        local_116 = (_e471 - vec3<f32>(0f, 0f, 0.5f));
    }
    let _e478: vec3<f32> = local_116;
    let _e479: vec3<f32> = glowAt(_e478);
    glow_3 = _e479;
    let _e481: vec3<i32> = open;
    let _e483: vec3<i32> = open;
    let _e485: vec3<i32> = open;
    let _e487: f32 = skyOpen(_e481.x, _e483.y, _e485.z);
    let _e488: f32 = canopy_5;
    let _e490: f32 = sun_6;
    let _e491: vec3<f32> = glow_3;
    let _e492: vec3<f32> = ladderLight((_e487 * _e488), _e490, _e491);
    here = _e492;
    let _e494: vec3<f32> = here;
    let _e495: f32 = sunOn_2;
    let _e496: vec3<f32> = ladderRef(_e495);
    ratio = (_e494 / max(_e496, vec3(0.0001f)));
    let _e502: vec3<f32> = colour_1;
    let _e503: vec3<f32> = wScatter;
    let _e504: vec3<f32> = ratio;
    let _e506: f32 = wHaze;
    colour_1 = (_e502 + ((_e503 * _e504) + (_e506 * u.hazeC.xyz)));
    let _e513: bool = top_14;
    if !(_e513) {
        {
            let _e515: vec3<f32> = colour_1;
            return _e515;
        }
    }
    let _e517: vec3<f32> = p_31;
    let _e519: vec3<f32> = fl_2;
    let _e523: vec3<f32> = rippleNormal(_e517.xz, _e519.xy, (&glint_1));
    n_6 = _e523;
    let _e526: f32 = k_9;
    d_6 = normalize(vec3<f32>(0f, -(_e526), 1f));
    let _e532: vec3<f32> = d_6;
    let _e533: vec3<f32> = n_6;
    cosi = clamp(-(dot(_e532, _e533)), 0f, 1f);
    let _e546: f32 = cosi;
    f_6 = clamp((u.waterL.y * (0.02f + (0.98f * pow((1f - _e546), 5f)))), 0f, 1f);
    let _e557: vec3<f32> = d_6;
    let _e558: vec3<f32> = n_6;
    r_25 = reflect(_e557, _e558);
    let _e562: vec3<f32> = r_25;
    r_25.y = max(_e562.y, 0.05f);
    let _e566: vec3<f32> = r_25;
    r_25 = normalize(_e566);
    if REFLECTIONS {
        {
            let _e569: f32 = f_6;
            let _e571: vec3<f32> = colour_1;
            under = ((1f - _e569) * _e571);
            let _e577: vec3<f32> = p_31;
            let _e578: vec3<f32> = r_25;
            let _e585: bool = reflectMarch(_e577, _e578, (&hc), (&hp_9), (&axis_8));
            found_2 = _e585;
            let _e587: vec3<f32> = under;
            let _e588: f32 = f_6;
            let _e589: bool = found_2;
            let _e590: vec3<i32> = hc;
            let _e591: vec3<f32> = hp_9;
            let _e592: i32 = axis_8;
            let _e593: vec3<f32> = r_25;
            let _e594: vec3<f32> = reflectionShade(_e589, _e590, _e591, _e592, _e593);
            colour_1 = (_e587 + (_e588 * _e594));
        }
    }
    let _e597: i32 = z_91;
    let _e601: f32 = hazeAt((f32(_e597) + 0.5f));
    hz_5 = _e601;
    surf = (u.waterSurfaceC.xyz * u.dayL.x);
    let _e611: vec3<f32> = colour_1;
    let _e612: vec3<f32> = surf;
    let _e613: f32 = hz_5;
    let _e614: vec3<f32> = hazed(_e612, _e613);
    froth = max(_e611, _e614);
    let _e628: vec3<i32> = open;
    let _e630: vec3<i32> = open;
    let _e632: vec3<i32> = open;
    let _e634: f32 = skyOpen(_e628.x, _e630.y, _e632.z);
    if (((u.waterM.z > 0f) && (u.rainK.w > 0f)) && (_e634 > 0f)) {
        {
            let _e638: vec3<f32> = p_31;
            let _e641: f32 = rainRing(_e638.xz, 0i);
            let _e642: vec3<f32> = p_31;
            let _e645: f32 = rainRing(_e642.xz, 1i);
            ring = max(_e641, _e645);
            let _e648: vec3<f32> = colour_1;
            let _e649: vec3<f32> = surf;
            let _e659: f32 = hz_5;
            let _e660: vec3<f32> = hazed(mix(_e649, vec3(u.dayL.x), vec3(u.waterM.w)), _e659);
            ringC = max(_e648, _e660);
            let _e663: vec3<f32> = colour_1;
            let _e664: vec3<f32> = ringC;
            let _e668: f32 = ring;
            colour_1 = mix(_e663, _e664, vec3(clamp((u.waterM.z * _e668), 0f, 1f)));
        }
    }
    if (u.waterM.x > 0f) {
        {
            let _e680: i32 = x_91;
            let _e681: i32 = y_81;
            let _e682: i32 = z_91;
            let _e684: vec3<f32> = p_31;
            let _e685: i32 = fill_5;
            let _e686: f32 = foamAt(vec3<i32>(_e680, _e681, _e682), _e684, _e685);
            foam = _e686;
            let _e688: vec3<f32> = colour_1;
            let _e689: vec3<f32> = froth;
            let _e693: f32 = foam;
            colour_1 = mix(_e688, _e689, vec3(clamp((u.waterM.x * _e693), 0f, 1f)));
        }
    }
    let _e705: f32 = glint_1;
    let _e709: f32 = sun_6;
    if (((u.waterM.y > 0f) && (_e705 > 0f)) && (_e709 > 0f)) {
        {
            let _e723: f32 = hz_5;
            let _e724: vec3<f32> = hazed(mix(u.sunLean.xyz, vec3(1f), vec3(u.waterM.w)), _e723);
            glintC = _e724;
            let _e726: vec3<f32> = colour_1;
            let _e727: vec3<f32> = glintC;
            let _e731: f32 = glint_1;
            colour_1 = mix(_e726, _e727, vec3(clamp(((u.waterM.y * _e731) * u.dayL.y), 0f, 1f)));
        }
    }
    let _e742: vec3<f32> = colour_1;
    return _e742;
}

fn wxTime() -> f32 {
    return u.cloudK.w;
}

fn wholeIn3600_(speed_1: f32, period_2: f32) -> f32 {
    var speed_2: f32;
    var period_3: f32;

    speed_2 = speed_1;
    period_3 = period_2;
    let _e224: f32 = speed_2;
    let _e227: f32 = period_3;
    let _e233: f32 = period_3;
    return ((max(1f, floor((((_e224 * 3600f) / _e227) + 0.5f))) * _e233) / 3600f);
}

fn fogShape(y_82: f32) -> f32 {
    var y_83: f32;
    var h_12: f32;
    var local_117: f32;

    y_83 = y_82;
    let _e221: f32 = y_83;
    h_12 = (_e221 - (u.fogK.y + u.fogK.z));
    let _e231: f32 = h_12;
    if (_e231 <= 0f) {
        local_117 = 1f;
    } else {
        let _e235: f32 = h_12;
        local_117 = exp((-(_e235) / max(u.fogK.w, 0.001f)));
    }
    let _e245: f32 = local_117;
    return _e245;
}

fn fogNoise(x_92: f32, z_92: f32) -> f32 {
    var x_93: f32;
    var z_93: f32;
    var mode: i32;
    var local_118: f32;
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
    z_93 = z_92;
    mode = i32((u.fogM.x + 0.5f));
    let _e233: i32 = mode;
    if (_e233 >= 2i) {
        let _e236: f32 = wxTime();
        let _e240: i32 = W;
        let _e242: f32 = wholeIn3600_(u.fogM.w, f32(_e240));
        let _e243: f32 = (_e236 * _e242);
        let _e244: i32 = W;
        let _e245: f32 = f32(_e244);
        local_118 = (_e243 - (floor((_e243 / _e245)) * _e245));
    } else {
        local_118 = 0f;
    }
    let _e252: f32 = local_118;
    flow_4 = _e252;
    let _e254: f32 = x_93;
    let _e259: f32 = flow_4;
    let _e261: f32 = z_93;
    w_16 = vec2<f32>(((_e254 - u.cloudK.y) - _e259), (_e261 * 1.6f));
    let _e267: vec2<f32> = w_16;
    let _e268: f32 = wxTime();
    let _e271: f32 = vnoiseT(_e267, FOG_FREQ, (_e268 * FOG_EVOLVE), 140i);
    let _e274: vec2<f32> = w_16;
    let _e280: f32 = vnoise(_e274, 0.25555554f, 7i, 141i);
    n_7 = ((0.65f * _e271) + (0.35f * _e280));
    let _e286: f32 = n_7;
    d_7 = (0.35f + (1.3f * _e286));
    let _e290: i32 = mode;
    if (_e290 >= 1i) {
        {
            let _e293: f32 = x_93;
            let _e294: f32 = flow_4;
            let _e296: f32 = z_93;
            wb_1 = vec2<f32>((_e293 - _e294), (_e296 * 1.6f));
            let _e302: vec2<f32> = wb_1;
            let _e305: f32 = vnoise(_e302, FOG_BANK_FREQ, 11i, 142i);
            a_23 = ((2f * _e305) - 1f);
            let _e311: vec2<f32> = wb_1;
            let _e314: f32 = vnoise(_e311, FOG_BANK_FREQ, 12i, 143i);
            b_14 = ((2f * _e314) - 1f);
            per = (3600f / max(1f, floor(((3600f / max(u.fogM.z, 1f)) + 0.5f))));
            let _e335: f32 = wxTime();
            let _e337: f32 = per;
            ph_1 = ((6.2831855f * _e335) / _e337);
            let _e340: f32 = d_7;
            let _e346: f32 = a_23;
            let _e347: f32 = ph_1;
            let _e350: f32 = b_14;
            let _e351: f32 = ph_1;
            d_7 = (_e340 * max(0f, (1f + (u.fogM.y * ((_e346 * cos(_e347)) + (_e350 * sin(_e351)))))));
        }
    }
    let _e359: f32 = d_7;
    return _e359;
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
    top_15 = (u.fogK.y + u.fogK.z);
    let _e236: f32 = y0_2;
    let _e237: f32 = top_15;
    let _e239: f32 = k_11;
    let _e242: f32 = zEnd_1;
    zb = clamp(((_e236 - _e237) / _e239), 0f, _e242);
    let _e245: f32 = zb;
    let _e246: f32 = zEnd_1;
    let _e247: f32 = zb;
    return vec2<f32>(_e245, (_e246 - _e247));
}

fn fogDepth(y0_3: f32, k_12: f32, zEnd_2: f32, noise: f32) -> f32 {
    var y0_4: f32;
    var k_13: f32;
    var zEnd_3: f32;
    var noise_1: f32;
    var span_2: vec2<f32>;
    var top_16: f32;
    var fw: f32;
    var local_119: f32;
    var above: f32;

    y0_4 = y0_3;
    k_13 = k_12;
    zEnd_3 = zEnd_2;
    noise_1 = noise;
    let _e230: f32 = y0_4;
    let _e231: f32 = k_13;
    let _e232: f32 = zEnd_3;
    let _e233: vec2<f32> = fogSpan(_e230, _e231, _e232);
    span_2 = _e233;
    top_16 = (u.fogK.y + u.fogK.z);
    fw = max(u.fogK.w, 0.001f);
    let _e249: f32 = y0_4;
    let _e250: f32 = top_16;
    if (_e249 > _e250) {
        let _e252: f32 = fw;
        let _e253: f32 = k_13;
        let _e255: f32 = y0_4;
        let _e256: f32 = k_13;
        let _e257: vec2<f32> = span_2;
        let _e261: f32 = top_16;
        let _e264: f32 = fw;
        let _e267: f32 = y0_4;
        let _e268: f32 = top_16;
        let _e271: f32 = fw;
        local_119 = ((_e252 / _e253) * (exp((-(((_e255 - (_e256 * _e257.x)) - _e261)) / _e264)) - exp((-((_e267 - _e268)) / _e271))));
    } else {
        local_119 = 0f;
    }
    let _e278: f32 = local_119;
    above = _e278;
    let _e283: f32 = noise_1;
    let _e285: f32 = above;
    let _e286: vec2<f32> = span_2;
    let _e291: f32 = k_13;
    let _e292: f32 = k_13;
    return (((u.fogK.x * _e283) * (_e285 + _e286.y)) * sqrt((1f + (_e291 * _e292))));
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
    let _e230: f32 = y0_6;
    let _e231: f32 = k_15;
    let _e232: f32 = zEnd_5;
    let _e233: vec2<f32> = fogSpan(_e230, _e231, _e232);
    span_3 = _e233;
    let _e235: f32 = xw_2;
    let _e236: vec2<f32> = span_3;
    let _e239: vec2<f32> = span_3;
    let _e243: f32 = fogNoise(_e235, (_e236.x + (0.5f * _e239.y)));
    return _e243;
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
    var local_120: f32;
    var above_1: f32;

    xw_4 = xw_3;
    y0_8 = y0_7;
    k_17 = k_16;
    zEnd_7 = zEnd_6;
    base_15 = (u.fogK.y + u.fogK.z);
    let _e239: f32 = xw_4;
    let _e240: f32 = y0_8;
    let _e241: f32 = base_15;
    let _e243: f32 = k_17;
    let _e246: f32 = zEnd_7;
    let _e248: f32 = fogNoise(_e239, clamp(((_e240 - _e241) / _e243), 0f, _e246));
    nb = _e248;
    let _e250: f32 = base_15;
    let _e251: f32 = nb;
    top_17 = (_e250 + (FOG_BILLOW * (_e251 - 1f)));
    let _e257: f32 = y0_8;
    let _e258: f32 = top_17;
    let _e260: f32 = k_17;
    let _e263: f32 = zEnd_7;
    zb_1 = clamp(((_e257 - _e258) / _e260), 0f, _e263);
    let _e266: f32 = xw_4;
    let _e267: f32 = zEnd_7;
    let _e268: f32 = fogNoise(_e266, _e267);
    ne = _e268;
    let _e270: f32 = zEnd_7;
    let _e271: f32 = zb_1;
    band_1 = (_e270 - _e271);
    fw_1 = max(u.fogK.w, 0.001f);
    let _e280: f32 = y0_8;
    let _e281: f32 = top_17;
    if (_e280 > _e281) {
        let _e283: f32 = fw_1;
        let _e284: f32 = k_17;
        let _e286: f32 = y0_8;
        let _e287: f32 = k_17;
        let _e288: f32 = zb_1;
        let _e291: f32 = top_17;
        let _e294: f32 = fw_1;
        let _e297: f32 = y0_8;
        let _e298: f32 = top_17;
        let _e301: f32 = fw_1;
        local_120 = ((_e283 / _e284) * (exp((-(((_e286 - (_e287 * _e288)) - _e291)) / _e294)) - exp((-((_e297 - _e298)) / _e301))));
    } else {
        local_120 = 0f;
    }
    let _e308: f32 = local_120;
    above_1 = _e308;
    let _e313: f32 = nb;
    let _e314: f32 = above_1;
    let _e316: f32 = band_1;
    let _e320: f32 = ne;
    let _e323: f32 = band_1;
    let _e328: f32 = k_17;
    let _e329: f32 = k_17;
    return ((u.fogK.x * ((_e313 * (_e314 + (0.5f * _e316))) + ((_e320 * 0.5f) * _e323))) * sqrt((1f + (_e328 * _e329))));
}

fn gustAt(xv: f32) -> f32 {
    var xv_1: f32;
    var t_38: f32;
    var n1_: f32;
    var n2_1: f32;
    var a_24: f32;
    var b_15: f32;

    xv_1 = xv;
    let _e239: f32 = wxTime();
    t_38 = _e239;
    let _e242: i32 = W;
    n1_ = max(1f, floor(((f32(_e242) / 38f) + 0.5f)));
    let _e252: i32 = W;
    n2_1 = max(1f, floor(((f32(_e252) / 17f) + 0.5f)));
    let _e262: f32 = xv_1;
    let _e263: f32 = n1_;
    let _e265: i32 = W;
    let _e268: f32 = t_38;
    a_24 = sin((6.2831855f * (((_e262 * _e263) / f32(_e265)) - (_e268 * 0.05f))));
    let _e276: f32 = xv_1;
    let _e277: f32 = n2_1;
    let _e279: i32 = W;
    let _e282: f32 = t_38;
    b_15 = sin(((6.2831855f * (((_e276 * _e277) / f32(_e279)) - (_e282 * 0.11f))) + 1.7f));
    let _e294: f32 = a_24;
    let _e298: f32 = b_15;
    return mix(GUST_LOW, 1f, clamp(((0.5f + (0.35f * _e294)) + (0.25f * _e298)), 0f, 1f));
}

fn rainStreak(px_7: f32, py_8: f32, mode_1: i32, layer_2: i32) -> f32 {
    var px_8: f32;
    var py_9: f32;
    var mode_2: i32;
    var layer_3: i32;
    var Sf: f32;
    var local_121: vec3<f32> = RAIN_SPEED;
    var local_122: vec3<f32> = RAIN_LAYER_SPEED;
    var speed_3: f32;
    var local_123: vec3<f32> = RAIN_LEN;
    var local_124: vec3<f32> = RAIN_LAYER_SPEED;
    var len_1: f32;
    var period_4: f32;
    var ringPx_1: f32;
    var local_125: vec3<f32> = RAIN_LANE;
    var lanes: f32;
    var laneW: f32;
    var local_126: vec3<f32> = RAIN_SLANT;
    var xs_1: f32;
    var lf: f32;
    var lane: i32;
    var h_13: u32;
    var off: f32;
    var width: f32;
    var cx: f32;
    var s_2: f32;
    var yp_2: f32;
    var cell_2: f32;
    var hc_1: u32;
    var local_127: vec3<f32> = RAIN_FILL;
    var pos: f32;
    var l_7: f32;
    var cy: f32;

    px_8 = px_7;
    py_9 = py_8;
    mode_2 = mode_1;
    layer_3 = layer_2;
    let _e245: i32 = S;
    Sf = f32(_e245);
    let _e248: i32 = mode_2;
    let _e252: f32 = local_121[_e248];
    let _e253: i32 = layer_3;
    let _e257: f32 = local_122[_e253];
    speed_3 = (_e252 * _e257);
    let _e260: i32 = mode_2;
    let _e264: f32 = local_123[_e260];
    let _e266: i32 = layer_3;
    let _e270: f32 = local_124[_e266];
    len_1 = (_e264 * mix(1f, _e270, 0.6f));
    let _e275: f32 = len_1;
    period_4 = (_e275 / 0.4f);
    let _e279: i32 = W;
    let _e281: f32 = Sf;
    ringPx_1 = (f32(_e279) * _e281);
    let _e285: f32 = ringPx_1;
    let _e286: i32 = mode_2;
    let _e290: f32 = local_125[_e286];
    let _e291: f32 = Sf;
    lanes = max(1f, floor(((_e285 / (_e290 * _e291)) + 0.5f)));
    let _e299: f32 = ringPx_1;
    let _e300: f32 = lanes;
    laneW = (_e299 / _e300);
    let _e303: f32 = px_8;
    let _e306: i32 = mode_2;
    let _e310: f32 = local_126[_e306];
    let _e311: f32 = py_9;
    xs_1 = ((_e303 + 0.5f) - (_e310 * (_e311 + 0.5f)));
    let _e317: f32 = xs_1;
    let _e318: f32 = laneW;
    lf = floor((_e317 / _e318));
    let _e322: f32 = lf;
    let _e324: f32 = lanes;
    let _e326: i32 = wrapI(i32(_e322), i32(_e324));
    lane = _e326;
    let _e328: i32 = lane;
    let _e329: i32 = layer_3;
    let _e330: i32 = mode_2;
    let _e332: u32 = cellHash(_e328, _e329, _e330, 170i);
    h_13 = _e332;
    let _e336: u32 = h_13;
    let _e344: f32 = laneW;
    off = ((0.15f + ((0.7f * f32((_e336 & 255u))) / 255f)) * _e344);
    let _e348: f32 = Sf;
    width = max(1f, (_e348 / 10f));
    let _e354: f32 = width;
    let _e358: f32 = xs_1;
    let _e359: f32 = lf;
    let _e360: f32 = laneW;
    let _e363: f32 = off;
    cx = clamp((((0.5f * _e354) + 0.5f) - abs(((_e358 - (_e359 * _e360)) - _e363))), 0f, 1f);
    let _e371: f32 = cx;
    if (_e371 <= 0f) {
        {
            return 0f;
        }
    }
    let _e375: f32 = speed_3;
    let _e378: u32 = h_13;
    let _e390: f32 = period_4;
    let _e391: f32 = wholeIn3600_((_e375 * (0.85f + ((0.3f * f32(((_e378 >> 8u) & 255u))) / 255f))), _e390);
    s_2 = _e391;
    let _e393: f32 = py_9;
    let _e396: f32 = Sf;
    let _e398: f32 = wxTime();
    let _e399: f32 = s_2;
    let _e402: f32 = period_4;
    let _e403: u32 = h_13;
    yp_2 = ((((_e393 + 0.5f) / _e396) - (_e398 * _e399)) + ((_e402 * f32(((_e403 >> 16u) & 255u))) / 255f));
    let _e415: f32 = yp_2;
    let _e416: f32 = period_4;
    cell_2 = floor((_e415 / _e416));
    let _e420: i32 = lane;
    let _e421: f32 = cell_2;
    let _e423: i32 = layer_3;
    let _e426: i32 = mode_2;
    let _e429: u32 = cellHash(_e420, i32(_e421), ((_e423 * 3i) + _e426), 171i);
    hc_1 = _e429;
    let _e431: u32 = hc_1;
    let _e437: i32 = mode_2;
    let _e441: f32 = local_127[_e437];
    if ((f32((_e431 & 65535u)) / 65535f) >= _e441) {
        {
            return 0f;
        }
    }
    let _e444: f32 = yp_2;
    let _e445: f32 = cell_2;
    let _e446: f32 = period_4;
    pos = (_e444 - (_e445 * _e446));
    let _e450: f32 = len_1;
    let _e453: u32 = hc_1;
    l_7 = (_e450 * (0.75f + ((0.5f * f32(((_e453 >> 16u) & 255u))) / 255f)));
    let _e466: f32 = l_7;
    let _e467: f32 = pos;
    let _e469: f32 = Sf;
    let _e474: f32 = pos;
    let _e475: f32 = Sf;
    cy = (clamp(((_e466 - _e467) * _e469), 0f, 1f) * clamp(((_e474 * _e475) + 0.5f), 0f, 1f));
    let _e484: f32 = cx;
    let _e485: f32 = cy;
    let _e489: f32 = pos;
    let _e490: f32 = l_7;
    return ((_e484 * _e485) * (0.35f + (0.65f * clamp((_e489 / _e490), 0f, 1f))));
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
    let _e245: f32 = y0_10;
    let _e246: f32 = k_19;
    let _e247: f32 = zl_1;
    y_84 = (_e245 - (_e246 * _e247));
    let _e251: f32 = y_84;
    let _e252: i32 = H;
    if (_e251 >= f32(_e252)) {
        {
            return true;
        }
    }
    let _e256: f32 = y_84;
    if (_e256 < 0f) {
        {
            return false;
        }
    }
    let _e260: f32 = y_84;
    yi = i32(_e260);
    let _e263: f32 = zl_1;
    zi = i32(_e263);
    let _e266: i32 = x_95;
    let _e267: i32 = yi;
    let _e268: i32 = zi;
    let _e269: bool = solidAt(_e266, _e267, _e268);
    let _e271: i32 = x_95;
    let _e272: i32 = yi;
    let _e273: i32 = zi;
    let _e274: i32 = roofGap(_e271, _e272, _e273);
    return (!(_e269) && (_e274 == 0i));
}

fn splashAt(w_17: vec2<f32>, copy_2: i32) -> f32 {
    var w_18: vec2<f32>;
    var copy_3: i32;
    var tt_1: f32;
    var slot_10: f32;
    var phase_3: f32;
    var c_54: vec2<i32>;
    var h_14: u32;
    var chance_1: f32;
    var hv_1: f32;
    var age_2: f32;
    var at_3: vec2<f32>;
    var e_10: vec2<f32>;
    var d_8: f32;
    var r_26: f32;
    var ring_1: f32;
    var dot_: f32;
    var fade_2: f32;

    w_18 = w_17;
    copy_3 = copy_2;
    let _e244: f32 = wxTime();
    let _e247: i32 = copy_3;
    tt_1 = ((_e244 / SPLASH_SLOT) + (0.5f * f32(_e247)));
    let _e252: f32 = tt_1;
    slot_10 = floor(_e252);
    let _e255: f32 = tt_1;
    let _e256: f32 = slot_10;
    phase_3 = (_e255 - _e256);
    let _e259: vec2<f32> = w_18;
    let _e261: i32 = copy_3;
    c_54 = vec2<i32>(floor((_e259 + vec2((0.5f * f32(_e261))))));
    let _e269: vec2<i32> = c_54;
    let _e271: i32 = W;
    let _e272: i32 = wrapI(_e269.x, _e271);
    let _e273: vec2<i32> = c_54;
    let _e275: f32 = slot_10;
    let _e278: i32 = copy_3;
    let _e280: u32 = cellHash(_e272, _e273.y, i32(_e275), (180i + _e278));
    h_14 = _e280;
    chance_1 = dot(u.rainK.xyz, SPLASH_CHANCE);
    let _e287: u32 = h_14;
    hv_1 = (f32((_e287 & 65535u)) / 65535f);
    let _e294: f32 = hv_1;
    let _e295: f32 = chance_1;
    if (_e294 >= _e295) {
        {
            return 0f;
        }
    }
    let _e298: f32 = phase_3;
    let _e299: u32 = h_14;
    age_2 = ((_e298 - ((f32(((_e299 >> 16u) & 255u)) / 255f) * 0.64f)) / SPLASH_LIFE);
    let _e315: f32 = age_2;
    let _e318: f32 = age_2;
    if ((_e315 <= 0f) || (_e318 >= 1f)) {
        {
            return 0f;
        }
    }
    let _e326: u32 = h_14;
    let _e333: u32 = h_14;
    at_3 = (vec2(0.2f) + ((0.6f * vec2<f32>(f32(((_e326 >> 24u) & 15u)), f32(((_e333 >> 28u) & 15u)))) / vec2(15f)));
    let _e347: vec2<f32> = w_18;
    let _e349: i32 = copy_3;
    let _e354: vec2<i32> = c_54;
    let _e357: vec2<f32> = at_3;
    let _e359: i32 = S;
    let _e361: i32 = RISE;
    e_10 = ((((_e347 + vec2((0.5f * f32(_e349)))) - vec2<f32>(_e354)) - _e357) * vec2<f32>(f32(_e359), f32(_e361)));
    let _e366: vec2<f32> = e_10;
    d_8 = length(_e366);
    let _e371: f32 = age_2;
    let _e375: i32 = S;
    r_26 = ((0.08f + (0.22f * sqrt(_e371))) * f32(_e375));
    let _e380: f32 = d_8;
    let _e381: f32 = r_26;
    let _e390: f32 = age_2;
    ring_1 = (max((1f - (abs((_e380 - _e381)) / 0.9f)), 0f) * (1f - _e390));
    let _e395: f32 = d_8;
    let _e404: f32 = age_2;
    dot_ = (max((1f - (_e395 / 1.2f)), 0f) * (1f - smoothstep(0f, 0.35f, _e404)));
    let _e411: f32 = age_2;
    let _e413: f32 = chance_1;
    let _e414: f32 = hv_1;
    fade_2 = (smoothstep(0f, 0.08f, _e411) * clamp(((_e413 - _e414) / 0.08f), 0f, 1f));
    let _e423: f32 = ring_1;
    let _e424: f32 = dot_;
    let _e426: f32 = fade_2;
    return (max(_e423, _e424) * _e426);
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
    let _e250: vec2<f32> = p_33;
    let _e251: vec2<f32> = a_26;
    pa_1 = (_e250 - _e251);
    let _e254: vec2<f32> = b_17;
    let _e255: vec2<f32> = a_26;
    ba_1 = (_e254 - _e255);
    let _e258: vec2<f32> = pa_1;
    let _e259: vec2<f32> = ba_1;
    let _e261: vec2<f32> = ba_1;
    let _e262: vec2<f32> = ba_1;
    t_39 = clamp((dot(_e258, _e259) / max(dot(_e261, _e262), 0.0001f)), 0f, 1f);
    let _e271: vec2<f32> = pa_1;
    let _e272: vec2<f32> = ba_1;
    let _e273: f32 = t_39;
    return length((_e271 - (_e272 * _e273)));
}

fn boltHash(i_13: i32, salt_18: i32) -> f32 {
    var i_14: i32;
    var salt_19: i32;
    var h_15: u32;

    i_14 = i_13;
    salt_19 = salt_18;
    let _e256: i32 = i_14;
    let _e257: i32 = salt_19;
    let _e258: u32 = cellHash(i32(u.strikeL.x), i32(u.strikeL.y), _e256, _e257);
    h_15 = _e258;
    let _e260: u32 = h_15;
    return ((f32((_e260 & 65535u)) / 65535f) - 0.5f);
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
    let _e253: vec2<f32> = a_28;
    let _e254: vec2<f32> = b_19;
    let _e257: f32 = kink_1;
    m_18 = ((0.5f * (_e253 + _e254)) + vec2<f32>(_e257, 0f));
    let _e262: vec2<f32> = p_35;
    let _e263: vec2<f32> = a_28;
    let _e264: vec2<f32> = m_18;
    let _e265: f32 = segDist(_e262, _e263, _e264);
    let _e266: vec2<f32> = p_35;
    let _e267: vec2<f32> = m_18;
    let _e268: vec2<f32> = b_19;
    let _e269: f32 = segDist(_e266, _e267, _e268);
    return min(_e265, _e269);
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
    var local_128: f32;
    var side: f32;
    var local_129: vec2<f32>;
    var fa: vec2<f32>;
    var j_2: i32;
    var fb: vec2<f32>;
    var w_19: f32;
    var core: f32;
    var glow_4: f32;
    var coreC: vec3<f32> = vec3<f32>(0.92f, 0.9f, 1f);
    var glowC: vec3<f32>;

    p_37 = p_36;
    let _e246: i32 = S;
    Sf_1 = f32(_e246);
    let _e252: f32 = Sf_1;
    let _e254: i32 = BASE;
    let _e259: i32 = RISE;
    let _e268: f32 = Sf_1;
    let _e272: i32 = RISE;
    end = vec2<f32>((u.strikeK.x * _e252), (((f32(_e254) - (u.strikeK.z * f32(_e259))) - ((u.strikeK.y + 1f) * _e268)) - (0.5f * f32(_e272))));
    let _e278: i32 = W;
    let _e280: f32 = Sf_1;
    ringPx_2 = (f32(_e278) * _e280);
    let _e284: vec2<f32> = end;
    let _e286: f32 = ringPx_2;
    let _e287: vec2<f32> = p_37;
    let _e289: vec2<f32> = end;
    let _e292: f32 = ringPx_2;
    end.x = (_e284.x + (_e286 * floor((((_e287.x - _e289.x) / _e292) + 0.5f))));
    let _e299: vec2<f32> = p_37;
    let _e301: vec2<f32> = end;
    let _e306: f32 = Sf_1;
    let _e309: vec2<f32> = p_37;
    let _e311: vec2<f32> = end;
    let _e314: f32 = Sf_1;
    if ((abs((_e299.x - _e301.x)) > (24f * _e306)) || (_e309.y > (_e311.y + (2f * _e314)))) {
        {
            return vec3(0f);
        }
    }
    let _e321: vec2<f32> = end;
    let _e325: f32 = boltHash(0i, 192i);
    let _e328: f32 = Sf_1;
    top_18 = vec2<f32>((_e321.x + ((_e325 * 8f) * _e328)), (-0.03f * f32(u.extent.w)));
    let _e340: vec2<f32> = end;
    let _e342: vec2<f32> = top_18;
    segH = ((_e340.y - _e342.y) / 18f);
    let _e349: f32 = segH;
    swing = ((BOLT_SWING * _e349) * 2f);
    loop {
        let _e358: i32 = j_1;
        if !((_e358 <= BOLT_SEGS)) {
            break;
        }
        {
            let _e364: f32 = drift;
            let _e365: i32 = j_1;
            let _e367: f32 = boltHash(_e365, 190i);
            let _e368: f32 = swing;
            drift = (_e364 + (_e367 * _e368));
        }
        continuing {
            let _e361: i32 = j_1;
            j_1 = (_e361 + 1i);
        }
    }
    let _e374: f32 = boltHash(0i, 193i);
    forkAt0_ = (3i + i32(((_e374 + 0.5f) * 5f)));
    let _e382: i32 = forkAt0_;
    let _e387: f32 = boltHash(1i, 193i);
    forkAt1_ = ((_e382 + 3i) + i32(((_e387 + 0.5f) * 5f)));
    let _e395: vec2<f32> = top_18;
    fork0_ = _e395;
    let _e397: vec2<f32> = top_18;
    fork1_ = _e397;
    let _e401: vec2<f32> = top_18;
    a_29 = _e401;
    loop {
        let _e407: i32 = i_15;
        if !((_e407 <= BOLT_SEGS)) {
            break;
        }
        {
            let _e413: f32 = walk_2;
            let _e414: i32 = i_15;
            let _e416: f32 = boltHash(_e414, 190i);
            let _e417: f32 = swing;
            walk_2 = (_e413 + (_e416 * _e417));
            let _e420: i32 = i_15;
            f_7 = (f32(_e420) / 18f);
            let _e426: vec2<f32> = top_18;
            let _e428: f32 = segH;
            let _e429: i32 = i_15;
            y_85 = (_e426.y + (_e428 * f32(_e429)));
            let _e434: i32 = i_15;
            if (_e434 < BOLT_SEGS) {
                {
                    let _e436: f32 = y_85;
                    let _e437: i32 = i_15;
                    let _e439: f32 = boltHash(_e437, 191i);
                    let _e442: f32 = segH;
                    y_85 = (_e436 + ((_e439 * 0.5f) * _e442));
                }
            }
            let _e445: vec2<f32> = top_18;
            let _e447: f32 = walk_2;
            let _e449: vec2<f32> = end;
            let _e451: vec2<f32> = top_18;
            let _e454: f32 = drift;
            let _e456: f32 = f_7;
            let _e459: f32 = y_85;
            b_20 = vec2<f32>(((_e445.x + _e447) + (((_e449.x - _e451.x) - _e454) * _e456)), _e459);
            let _e462: vec2<f32> = p_37;
            let _e465: vec2<f32> = a_29;
            let _e467: vec2<f32> = b_20;
            let _e473: f32 = segH;
            let _e475: f32 = Sf_1;
            if (abs((_e462.y - (0.5f * (_e465.y + _e467.y)))) < (_e473 + (2f * _e475))) {
                {
                    let _e479: f32 = d_9;
                    let _e480: vec2<f32> = p_37;
                    let _e481: vec2<f32> = a_29;
                    let _e482: vec2<f32> = b_20;
                    let _e483: i32 = i_15;
                    let _e485: f32 = boltHash(_e483, 197i);
                    let _e489: f32 = segH;
                    let _e491: f32 = kinkDist(_e480, _e481, _e482, (((_e485 * 2f) * BOLT_KINK) * _e489));
                    d_9 = min(_e479, _e491);
                }
            }
            let _e493: i32 = i_15;
            let _e494: i32 = forkAt0_;
            if (_e493 == _e494) {
                {
                    let _e496: vec2<f32> = b_20;
                    fork0_ = _e496;
                }
            }
            let _e497: i32 = i_15;
            let _e498: i32 = forkAt1_;
            if (_e497 == _e498) {
                {
                    let _e500: vec2<f32> = b_20;
                    fork1_ = _e500;
                }
            }
            let _e501: vec2<f32> = b_20;
            a_29 = _e501;
        }
        continuing {
            let _e410: i32 = i_15;
            i_15 = (_e410 + 1i);
        }
    }
    loop {
        let _e506: i32 = f_8;
        if !((_e506 < 2i)) {
            break;
        }
        {
            let _e513: i32 = f_8;
            let _e515: f32 = boltHash(_e513, 194i);
            if (_e515 < 0f) {
                local_128 = -1f;
            } else {
                local_128 = 1f;
            }
            let _e522: f32 = local_128;
            side = _e522;
            let _e524: i32 = f_8;
            if (_e524 == 0i) {
                let _e527: vec2<f32> = fork0_;
                local_129 = _e527;
            } else {
                let _e528: vec2<f32> = fork1_;
                local_129 = _e528;
            }
            let _e530: vec2<f32> = local_129;
            fa = _e530;
            j_2 = 1i;
            loop {
                let _e534: i32 = j_2;
                if !((_e534 <= FORK_SEGS)) {
                    break;
                }
                {
                    let _e540: vec2<f32> = fa;
                    let _e541: f32 = side;
                    let _e545: i32 = f_8;
                    let _e547: i32 = j_2;
                    let _e550: f32 = boltHash(((10i * _e545) + _e547), 195i);
                    let _e558: f32 = segH;
                    let _e560: f32 = segH;
                    let _e564: i32 = f_8;
                    let _e566: i32 = j_2;
                    let _e569: f32 = boltHash(((10i * _e564) + _e566), 196i);
                    fb = (_e540 + vec2<f32>((((_e541 * (0.3f + (0.9f * (_e550 + 0.5f)))) * 0.6f) * _e558), (_e560 * (0.45f + (0.4f * (_e569 + 0.5f))))));
                    let _e578: f32 = fd;
                    let _e579: vec2<f32> = p_37;
                    let _e580: vec2<f32> = fa;
                    let _e581: vec2<f32> = fb;
                    let _e583: i32 = f_8;
                    let _e585: i32 = j_2;
                    let _e588: f32 = boltHash(((10i * _e583) + _e585), 198i);
                    let _e591: f32 = segH;
                    let _e593: f32 = kinkDist(_e579, _e580, _e581, ((_e588 * 0.3f) * _e591));
                    let _e594: i32 = j_2;
                    fd = min(_e578, (_e593 + (f32(_e594) * 0.4f)));
                    let _e600: vec2<f32> = fb;
                    fa = _e600;
                }
                continuing {
                    let _e537: i32 = j_2;
                    j_2 = (_e537 + 1i);
                }
            }
        }
        continuing {
            let _e510: i32 = f_8;
            f_8 = (_e510 + 1i);
        }
    }
    let _e602: f32 = Sf_1;
    w_19 = max(0.75f, (_e602 / 9f));
    let _e607: f32 = w_19;
    let _e610: f32 = d_9;
    let _e617: f32 = w_19;
    let _e621: f32 = fd;
    core = (clamp(((_e607 + 0.5f) - _e610), 0f, 1f) + (0.55f * clamp((((0.5f * _e617) + 0.5f) - _e621), 0f, 1f)));
    let _e629: f32 = d_9;
    let _e632: f32 = Sf_1;
    let _e637: f32 = fd;
    let _e640: f32 = Sf_1;
    glow_4 = (exp((-(_e629) / (0.9f * _e632))) + (0.4f * exp((-(_e637) / (0.6f * _e640)))));
    glowC = mix(u.lightC.xyz, vec3<f32>(0.62f, 0.42f, 1f), vec3(0.6f));
    let _e667: f32 = core;
    let _e669: vec3<f32> = coreC;
    let _e672: f32 = glow_4;
    let _e674: vec3<f32> = glowC;
    return (u.strikeK.w * (((2.2f * _e667) * _e669) + ((0.6f * _e672) * _e674)));
}

fn over(c_55: ptr<function, vec3<f32>>, a_30: ptr<function, f32>, lc_1: vec3<f32>, la: f32) {
    var lc_2: vec3<f32>;
    var la_1: f32;

    lc_2 = lc_1;
    la_1 = la;
    let _e250: vec3<f32> = (*c_55);
    let _e252: f32 = la_1;
    let _e255: vec3<f32> = lc_2;
    let _e256: f32 = la_1;
    (*c_55) = ((_e250 * (1f - _e252)) + (_e255 * _e256));
    let _e259: f32 = (*a_30);
    let _e260: f32 = la_1;
    let _e262: f32 = (*a_30);
    (*a_30) = (_e259 + (_e260 * (1f - _e262)));
    return;
}

fn weatherAt(px_9: vec2<i32>, zEnd_8: f32) -> vec4<f32> {
    var px_10: vec2<i32>;
    var zEnd_9: f32;
    var k_20: f32;
    var xw_5: f32;
    var y0_11: f32;
    var x_96: i32;
    var c_56: vec3<f32> = vec3(0f);
    var a_31: f32 = 0f;
    var local_130: f32;
    var t_40: f32;
    var fl_3: vec3<f32>;
    var zg: f32;
    var wsum: f32;
    var depth_2: f32;
    var local_131: f32;
    var gust: f32;
    var curtain: vec3<f32> = vec3(0f);
    var dark_1: f32 = 0f;
    var bg: vec3<f32>;
    var Y: vec3<f32> = vec3<f32>(0.2126f, 0.7152f, 0.0722f);
    var veil: f32;
    var zi_1: i32;
    var q_19: i32;
    var local_132: i32;
    var level_4: i32;
    var r_27: i32;
    var w_20: vec2<f32>;
    var sp_4: f32;
    var sc: vec3<f32>;
    var l_8: i32 = 2i;
    var local_133: vec3<f32> = RAIN_DEPTH;
    var zl_2: f32;
    var rc: vec3<f32>;
    var m_19: i32;
    var wm: f32;
    var st_5: f32;
    var local_134: f32;
    var g_5: f32;
    var local_135: vec3<f32> = RAIN_ALPHA;
    var local_136: vec3<f32> = RAIN_LAYER_ALPHA;
    var cover_2: f32;

    px_10 = px_9;
    zEnd_9 = zEnd_8;
    let _e252: i32 = RISE;
    let _e254: i32 = S;
    k_20 = (f32(_e252) / f32(_e254));
    let _e258: i32 = PX;
    let _e262: i32 = S;
    xw_5 = ((f32(_e258) + 0.5f) / f32(_e262));
    let _e266: i32 = BASE;
    let _e267: vec2<i32> = px_10;
    let _e275: i32 = S;
    y0_11 = ((f32(((_e266 - _e267.y) - 1i)) + 0.5f) / f32(_e275));
    let _e279: i32 = PX;
    let _e280: i32 = S;
    x_96 = (_e279 / _e280);
    if (!(VOLUMETRIC) && (u.fogK.x > 0f)) {
        {
            if (i32((u.fogM.x + 0.5f)) == 3i) {
                let _e303: f32 = xw_5;
                let _e304: f32 = y0_11;
                let _e305: f32 = k_20;
                let _e306: f32 = zEnd_9;
                let _e307: f32 = fogDepthDeep(_e303, _e304, _e305, _e306);
                local_130 = exp(-(_e307));
            } else {
                let _e310: f32 = y0_11;
                let _e311: f32 = k_20;
                let _e312: f32 = zEnd_9;
                let _e313: f32 = xw_5;
                let _e314: f32 = y0_11;
                let _e315: f32 = k_20;
                let _e316: f32 = zEnd_9;
                let _e317: f32 = fogNoiseFor(_e313, _e314, _e315, _e316);
                let _e318: f32 = fogDepth(_e310, _e311, _e312, _e317);
                local_130 = exp(-(_e318));
            }
            let _e322: f32 = local_130;
            t_40 = _e322;
            let _e324: f32 = t_40;
            if (_e324 < 0.999f) {
                {
                    fl_3 = (u.fogC.xyz + ((u.rainC.w * u.dayL.y) * u.sunLean.xyz));
                    if (GLOW && (u.fogC.w > 0f)) {
                        {
                            let _e349: f32 = zEnd_9;
                            zg = max((_e349 - 1.5f), 0.5f);
                            let _e355: vec3<f32> = fl_3;
                            let _e359: f32 = xw_5;
                            let _e360: f32 = y0_11;
                            let _e361: f32 = k_20;
                            let _e362: f32 = zg;
                            let _e367: f32 = zg;
                            let _e369: vec3<f32> = glowAt(vec3<f32>(_e359, max((_e360 - (_e361 * _e362)), 0.5f), _e367));
                            fl_3 = (_e355 + (u.fogC.w * _e369));
                        }
                    }
                    let _e374: vec3<f32> = fl_3;
                    let _e376: f32 = t_40;
                    over((&c_56), (&a_31), _e374, (1f - _e376));
                }
            }
        }
    }
    wsum = ((u.rainK.x + u.rainK.y) + u.rainK.z);
    let _e392: f32 = wsum;
    if ((_e392 > 0f) || (((u.rainH.x + u.rainH.y) + u.rainH.z) > 0f)) {
        {
            let _e409: f32 = zEnd_9;
            let _e410: i32 = D;
            let _e413: i32 = D;
            depth_2 = (min(_e409, f32(_e410)) / f32(max(_e413, 1i)));
            if ((u.rainK.z > 0f) || (u.rainH.z > 0f)) {
                let _e430: f32 = xw_5;
                let _e432: f32 = y0_11;
                let _e435: f32 = gustAt((_e430 - (0.24f * _e432)));
                local_131 = _e435;
            } else {
                local_131 = 1f;
            }
            let _e438: f32 = local_131;
            gust = _e438;
            let _e445: f32 = zEnd_9;
            let _e446: i32 = D;
            if (_e445 >= f32(_e446)) {
                {
                    let _e449: vec2<i32> = px_10;
                    let _e451: vec4<f32> = textureLoad(wgpu_backdropTex, _e449, 0i);
                    bg = _e451.xyz;
                    let _e464: vec3<f32> = Y;
                    let _e466: vec3<f32> = bg;
                    let _e467: vec3<f32> = Y;
                    dark_1 = (1f - smoothstep(0f, RAIN_CURTAIN_RAMP, (dot(u.rainC.xyz, _e464) - dot(_e466, _e467))));
                    let _e472: vec3<f32> = bg;
                    curtain = (_e472 * RAIN_CURTAIN);
                }
            }
            let _e479: f32 = gust;
            let _e483: f32 = depth_2;
            veil = (dot(u.rainH.xyz, (RAIN_VEIL * vec3<f32>(1f, 1f, _e479))) * _e483);
            let _e486: f32 = veil;
            if (_e486 > 0f) {
                {
                    let _e495: vec3<f32> = hazed(u.rainC.xyz, 0.5f);
                    let _e496: vec3<f32> = curtain;
                    let _e497: f32 = dark_1;
                    let _e500: f32 = veil;
                    over((&c_56), (&a_31), mix(_e495, _e496, vec3(_e497)), _e500);
                }
            }
            let _e503: f32 = zEnd_9;
            zi_1 = i32(_e503);
            let _e506: i32 = zi_1;
            let _e507: i32 = D;
            if (_e506 < _e507) {
                {
                    let _e509: i32 = BASE;
                    let _e510: i32 = zi_1;
                    let _e511: i32 = RISE;
                    let _e514: vec2<i32> = px_10;
                    q_19 = (((_e509 - (_e510 * _e511)) - _e514.y) - 1i);
                    let _e520: i32 = q_19;
                    if (_e520 >= 0i) {
                        let _e523: i32 = q_19;
                        let _e524: i32 = S;
                        local_132 = (_e523 / _e524);
                    } else {
                        local_132 = -1i;
                    }
                    let _e529: i32 = local_132;
                    level_4 = _e529;
                    let _e531: i32 = q_19;
                    let _e532: i32 = level_4;
                    let _e533: i32 = S;
                    r_27 = (_e531 - (_e532 * _e533));
                    let _e537: i32 = level_4;
                    let _e540: i32 = r_27;
                    let _e541: i32 = RISE;
                    let _e544: i32 = level_4;
                    let _e547: i32 = H;
                    let _e550: i32 = x_96;
                    let _e551: i32 = level_4;
                    let _e554: i32 = zi_1;
                    let _e555: bool = solidAt(_e550, (_e551 - 1i), _e554);
                    let _e557: i32 = level_4;
                    let _e558: i32 = H;
                    let _e560: i32 = x_96;
                    let _e561: i32 = level_4;
                    let _e562: i32 = zi_1;
                    let _e563: bool = solidAt(_e560, _e561, _e562);
                    let _e567: i32 = x_96;
                    let _e568: i32 = level_4;
                    let _e571: i32 = zi_1;
                    let _e572: i32 = roofGap(_e567, (_e568 - 1i), _e571);
                    if ((((((_e537 >= 1i) && (_e540 < _e541)) && ((_e544 - 1i) < _e547)) && _e555) && !(((_e557 < _e558) && _e563))) && (_e572 == 0i)) {
                        {
                            let _e576: f32 = xw_5;
                            let _e577: i32 = zi_1;
                            let _e581: i32 = RISE;
                            let _e584: i32 = r_27;
                            let _e589: i32 = RISE;
                            w_20 = vec2<f32>(_e576, ((f32(_e577) + 1f) - ((f32(((_e581 - 1i) - _e584)) + 0.5f) / f32(_e589))));
                            let _e595: vec2<f32> = w_20;
                            let _e597: f32 = splashAt(_e595, 0i);
                            let _e598: vec2<f32> = w_20;
                            let _e600: f32 = splashAt(_e598, 1i);
                            sp_4 = max(_e597, _e600);
                            let _e603: f32 = sp_4;
                            if (_e603 > 0f) {
                                {
                                    let _e616: f32 = zEnd_9;
                                    let _e617: f32 = hazeAt(_e616);
                                    let _e618: vec3<f32> = hazed(mix(u.rainC.xyz, vec3(u.dayL.x), vec3(0.15f)), _e617);
                                    sc = _e618;
                                    let _e622: vec3<f32> = sc;
                                    let _e624: f32 = sp_4;
                                    over((&c_56), (&a_31), _e622, (0.5f * _e624));
                                }
                            }
                        }
                    }
                }
            }
            loop {
                let _e630: i32 = l_8;
                if !((_e630 >= 0i)) {
                    break;
                }
                {
                    let _e637: i32 = l_8;
                    let _e641: f32 = local_133[_e637];
                    let _e642: i32 = D;
                    zl_2 = (_e641 * f32(_e642));
                    let _e646: f32 = zEnd_9;
                    let _e647: f32 = zl_2;
                    let _e649: i32 = x_96;
                    let _e650: f32 = y0_11;
                    let _e651: f32 = k_20;
                    let _e652: f32 = zl_2;
                    let _e653: bool = rainOpen(_e649, _e650, _e651, _e652);
                    if ((_e646 <= _e647) || !(_e653)) {
                        {
                            continue;
                        }
                    }
                    let _e659: f32 = zl_2;
                    let _e660: f32 = hazeAt(_e659);
                    let _e661: vec3<f32> = hazed(u.rainC.xyz, _e660);
                    let _e662: vec3<f32> = curtain;
                    let _e663: f32 = dark_1;
                    rc = mix(_e661, _e662, vec3(_e663));
                    m_19 = 0i;
                    loop {
                        let _e669: i32 = m_19;
                        if !((_e669 < 3i)) {
                            break;
                        }
                        {
                            let _e676: i32 = m_19;
                            let _e679: f32 = u.rainK[_e676];
                            wm = _e679;
                            let _e681: f32 = wm;
                            if (_e681 <= 0f) {
                                {
                                    continue;
                                }
                            }
                            let _e684: vec2<i32> = px_10;
                            let _e687: vec2<i32> = px_10;
                            let _e690: i32 = m_19;
                            let _e691: i32 = l_8;
                            let _e692: f32 = rainStreak(f32(_e684.x), f32(_e687.y), _e690, _e691);
                            st_5 = _e692;
                            let _e694: f32 = st_5;
                            if (_e694 <= 0f) {
                                {
                                    continue;
                                }
                            }
                            let _e697: i32 = m_19;
                            if (_e697 == 2i) {
                                let _e700: f32 = gust;
                                local_134 = _e700;
                            } else {
                                local_134 = 1f;
                            }
                            let _e703: f32 = local_134;
                            g_5 = _e703;
                            let _e705: f32 = st_5;
                            let _e706: f32 = wm;
                            let _e708: f32 = g_5;
                            let _e710: i32 = m_19;
                            let _e714: f32 = local_135[_e710];
                            let _e716: i32 = l_8;
                            let _e720: f32 = local_136[_e716];
                            cover_2 = ((((_e705 * _e706) * _e708) * _e714) * _e720);
                            let _e725: vec3<f32> = rc;
                            let _e726: f32 = cover_2;
                            let _e728: f32 = dark_1;
                            over((&c_56), (&a_31), _e725, clamp((_e726 * mix(1f, RAIN_CURTAIN_ALPHA, _e728)), 0f, 1f));
                        }
                        continuing {
                            let _e673: i32 = m_19;
                            m_19 = (_e673 + 1i);
                        }
                    }
                }
                continuing {
                    let _e634: i32 = l_8;
                    l_8 = (_e634 - 1i);
                }
            }
        }
    }
    let _e741: f32 = zEnd_9;
    if ((u.strikeK.w > 0f) && (_e741 >= u.strikeK.z)) {
        {
            let _e747: vec3<f32> = c_56;
            let _e748: vec2<i32> = px_10;
            let _e753: vec3<f32> = boltAt((vec2<f32>(_e748) + vec2(0.5f)));
            c_56 = (_e747 + _e753);
        }
    }
    let _e755: vec3<f32> = c_56;
    let _e756: f32 = a_31;
    return vec4<f32>(_e755.x, _e755.y, _e755.z, _e756);
}

fn weatherOver(px_11: vec2<i32>) -> vec4<f32> {
    var px_12: vec2<i32>;
    var walkZ: f32;
    var plant: vec4<f32> = vec4(0f);
    var figure: vec4<f32> = vec4(0f);
    var plantZ: f32;
    var figureZ: f32;
    var zEnd_10: f32;
    var farWeather: vec4<f32>;
    var local_137: vec4<f32>;
    var plantWeather: vec4<f32>;
    var local_138: vec4<f32>;
    var figureWeather: vec4<f32>;
    var behindFigure: f32;
    var light: vec3<f32>;

    px_12 = px_11;
    let _e250: vec2<i32> = px_12;
    let _e252: vec4<f32> = textureLoad(wgpu_depthTex, _e250, 0i);
    walkZ = floor(((_e252.w * 255f) + 0.5f));
    let _e266: f32 = walkZ;
    plantZ = _e266;
    let _e268: f32 = walkZ;
    figureZ = _e268;
    if ((u.camera.z & 1i) != 0i) {
        {
            let _e277: vec2<i32> = px_12;
            let _e279: vec4<f32> = textureLoad(wgpu_weatherPlantSurface, _e277, 0i);
            plant = _e279;
            let _e280: vec4<f32> = plant;
            if (_e280.w > 0f) {
                {
                    let _e285: f32 = walkZ;
                    let _e286: vec2<i32> = px_12;
                    let _e288: vec4<f32> = textureLoad(wgpu_weatherPlantZ, _e286, 0i);
                    plantZ = max(0f, min(_e285, _e288.x));
                }
            }
        }
    }
    if ((u.camera.z & 2i) != 0i) {
        {
            let _e299: vec2<i32> = px_12;
            let _e301: vec4<f32> = textureLoad(wgpu_weatherFigureSurface, _e299, 0i);
            figure = _e301;
            let _e302: vec4<f32> = figure;
            if (_e302.w > 0f) {
                {
                    let _e307: f32 = plantZ;
                    let _e308: vec2<i32> = px_12;
                    let _e310: f32 = textureLoad(wgpu_weatherFigureZ, _e308, 0i);
                    let _e312: i32 = D;
                    figureZ = max(0f, min(_e307, ((f32(_e310) * (f32(_e312) + 32f)) - 16f)));
                }
            }
        }
    }
    let _e321: f32 = walkZ;
    let _e322: f32 = plantZ;
    let _e323: f32 = figureZ;
    zEnd_10 = min(_e321, min(_e322, _e323));
    let _e327: vec4<f32> = figure;
    let _e331: vec4<f32> = figure;
    let _e335: vec4<f32> = plant;
    let _e341: f32 = zEnd_10;
    let _e342: f32 = walkZ;
    if (((_e327.w >= 1f) || ((_e331.w == 0f) && (_e335.w >= 1f))) || (_e341 == _e342)) {
        {
            let _e345: vec2<i32> = px_12;
            let _e346: f32 = zEnd_10;
            let _e347: vec4<f32> = weatherAt(_e345, _e346);
            return _e347;
        }
    }
    let _e348: vec2<i32> = px_12;
    let _e349: f32 = walkZ;
    let _e350: vec4<f32> = weatherAt(_e348, _e349);
    farWeather = _e350;
    let _e352: f32 = plantZ;
    let _e353: f32 = walkZ;
    if (_e352 == _e353) {
        let _e355: vec4<f32> = farWeather;
        local_137 = _e355;
    } else {
        let _e356: vec2<i32> = px_12;
        let _e357: f32 = plantZ;
        let _e358: vec4<f32> = weatherAt(_e356, _e357);
        local_137 = _e358;
    }
    let _e360: vec4<f32> = local_137;
    plantWeather = _e360;
    let _e362: f32 = figureZ;
    let _e363: f32 = plantZ;
    if (_e362 == _e363) {
        let _e365: vec4<f32> = plantWeather;
        local_138 = _e365;
    } else {
        let _e366: vec2<i32> = px_12;
        let _e367: f32 = figureZ;
        let _e368: vec4<f32> = weatherAt(_e366, _e367);
        local_138 = _e368;
    }
    let _e370: vec4<f32> = local_138;
    figureWeather = _e370;
    let _e373: vec4<f32> = figure;
    behindFigure = (1f - _e373.w);
    let _e377: f32 = behindFigure;
    let _e378: vec4<f32> = farWeather;
    let _e380: vec4<f32> = plantWeather;
    let _e382: vec4<f32> = plant;
    let _e387: vec4<f32> = figure;
    let _e389: vec4<f32> = figureWeather;
    light = ((_e377 * mix(_e378.xyz, _e380.xyz, vec3(_e382.w))) + (_e387.w * _e389.xyz));
    let _e394: vec3<f32> = light;
    let _e395: f32 = behindFigure;
    let _e396: vec4<f32> = plant;
    let _e399: vec4<f32> = farWeather;
    let _e401: vec4<f32> = plantWeather;
    let _e405: vec4<f32> = figure;
    let _e407: vec4<f32> = farWeather;
    let _e409: vec4<f32> = figureWeather;
    light = (_e394 + (((_e395 * _e396.xyz) * (_e399.w - _e401.w)) + (_e405.xyz * (_e407.w - _e409.w))));
    let _e415: vec3<f32> = light;
    let _e416: vec4<f32> = farWeather;
    return vec4<f32>(_e415.x, _e415.y, _e415.z, _e416.w);
}

fn flatRain(px_13: i32, py_10: i32, colour_2: vec3<f32>) -> vec3<f32> {
    var px_14: i32;
    var py_11: i32;
    var colour_3: vec3<f32>;
    var wsum_1: f32;
    var local_139: i32;
    var local_140: i32;
    var m_20: i32;
    var st_6: f32;
    var local_141: vec3<f32> = RAIN_ALPHA;

    px_14 = px_13;
    py_11 = py_10;
    colour_3 = colour_2;
    wsum_1 = ((u.rainK.x + u.rainK.y) + u.rainK.z);
    if (u.rainK.z >= max(u.rainK.x, u.rainK.y)) {
        local_140 = 2i;
    } else {
        if (u.rainK.y >= u.rainK.x) {
            local_139 = 1i;
        } else {
            local_139 = 0i;
        }
        let _e288: i32 = local_139;
        local_140 = _e288;
    }
    let _e290: i32 = local_140;
    m_20 = _e290;
    let _e292: i32 = px_14;
    let _e294: i32 = py_11;
    let _e296: i32 = m_20;
    let _e298: f32 = rainStreak(f32(_e292), f32(_e294), _e296, 0i);
    st_6 = _e298;
    let _e300: f32 = st_6;
    if (_e300 <= 0f) {
        {
            let _e303: vec3<f32> = colour_3;
            return _e303;
        }
    }
    let _e304: vec3<f32> = colour_3;
    let _e308: f32 = st_6;
    let _e309: f32 = wsum_1;
    let _e313: i32 = m_20;
    let _e317: f32 = local_141[_e313];
    return mix(_e304, u.rainC.xyz, vec3(clamp((((_e308 * min(_e309, 1f)) * _e317) * 0.8f), 0f, 1f)));
}

fn sunInAir(p_38: vec3<f32>) -> f32 {
    var p_39: vec3<f32>;
    var v_68: vec2<f32>;

    p_39 = p_38;
    let _e251: vec3<f32> = p_39;
    let _e252: i32 = W;
    let _e254: i32 = H;
    let _e256: i32 = D;
    let _e261: vec4<f32> = textureSampleLevel(wgpu_sunVisTex, wgpu_sampler_0_33_, (_e251 / vec3<f32>(f32(_e252), f32(_e254), f32(_e256))), 0f);
    v_68 = _e261.xy;
    let _e264: vec2<f32> = v_68;
    let _e266: vec2<f32> = v_68;
    return mix(_e264.x, _e266.y, u.volK.w);
}

fn fogDensity(p_40: vec3<f32>) -> f32 {
    var p_41: vec3<f32>;

    p_41 = p_40;
    if (u.fogK.x <= 0f) {
        {
            return 0f;
        }
    }
    let _e261: f32 = fogPixelNoise;
    let _e263: vec3<f32> = p_41;
    let _e265: f32 = fogShape(_e263.y);
    return ((u.fogK.x * _e261) * _e265);
}

fn airDensity(p_42: vec3<f32>) -> f32 {
    var p_43: vec3<f32>;

    p_43 = p_42;
    let _e255: vec3<f32> = p_43;
    let _e266: vec3<f32> = p_43;
    let _e267: f32 = fogDensity(_e266);
    return ((u.volK.x * exp((-(max(_e255.y, 0f)) / u.volK.z))) + _e267);
}

fn inScatter(zEnd_11: i32) -> vec4<f32> {
    var zEnd_12: i32;
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
    var light_1: vec3<f32>;
    var local_142: vec3<f32>;
    var glow_5: vec3<f32>;
    var here_1: vec3<f32>;

    zEnd_12 = zEnd_11;
    let _e253: i32 = zEnd_12;
    if (_e253 <= 0i) {
        {
            return vec4<f32>(0f, 0f, 0f, 1f);
        }
    }
    let _e261: i32 = RISE;
    let _e263: i32 = S;
    k_21 = (f32(_e261) / f32(_e263));
    let _e267: i32 = zEnd_12;
    n_8 = min(VOL_SAMPLES, _e267);
    let _e270: i32 = zEnd_12;
    let _e272: i32 = n_8;
    dz_2 = (f32(_e270) / f32(_e272));
    let _e276: f32 = dz_2;
    let _e278: f32 = k_21;
    let _e279: f32 = k_21;
    ds_1 = (_e276 * sqrt((1f + (_e278 * _e279))));
    let _e285: i32 = PX;
    let _e289: i32 = S;
    xw_6 = ((f32(_e285) + 0.5f) / f32(_e289));
    let _e293: i32 = BASE;
    let _e294: i32 = PY;
    let _e301: i32 = S;
    y0_12 = ((f32(((_e293 - _e294) - 1i)) + 0.5f) / f32(_e301));
    if (u.fogK.x > 0f) {
        {
            let _e310: f32 = xw_6;
            let _e311: f32 = y0_12;
            let _e312: f32 = k_21;
            let _e313: i32 = zEnd_12;
            let _e315: f32 = fogNoiseFor(_e310, _e311, _e312, f32(_e313));
            fogPixelNoise = _e315;
        }
    }
    loop {
        let _e323: i32 = i_16;
        let _e324: i32 = n_8;
        if !((_e323 < _e324)) {
            break;
        }
        {
            let _e330: i32 = i_16;
            let _e334: f32 = dz_2;
            zs = ((f32(_e330) + 0.5f) * _e334);
            let _e337: f32 = xw_6;
            let _e338: f32 = y0_12;
            let _e339: f32 = zs;
            let _e340: f32 = k_21;
            let _e343: f32 = zs;
            p_44 = vec3<f32>(_e337, (_e338 - (_e339 * _e340)), _e343);
            let _e346: vec3<f32> = p_44;
            let _e347: f32 = fogDensity(_e346);
            fog = _e347;
            let _e349: vec3<f32> = p_44;
            let _e350: f32 = airDensity(_e349);
            let _e351: f32 = fog;
            let _e354: f32 = zs;
            let _e355: f32 = hazeAt(_e354);
            d_10 = ((_e350 - _e351) * (1f - _e355));
            let _e365: vec3<f32> = p_44;
            let _e366: f32 = sunInAir(_e365);
            sun_7 = (u.sunLean.xyz * (u.dayL.y * _e366));
            let _e370: vec3<f32> = sun_7;
            light_1 = (_e370 + (0.5f * u.flashC.xyz));
            let _e383: f32 = fog;
            if (GLOW && ((u.volK.y > 0f) || (_e383 > 0f))) {
                let _e388: vec3<f32> = p_44;
                let _e389: vec3<f32> = glowAt(_e388);
                local_142 = _e389;
            } else {
                local_142 = vec3(0f);
            }
            let _e393: vec3<f32> = local_142;
            glow_5 = _e393;
            let _e395: vec3<f32> = light_1;
            let _e399: vec3<f32> = glow_5;
            light_1 = (_e395 + (u.volK.y * _e399));
            let _e402: f32 = d_10;
            let _e403: vec3<f32> = light_1;
            here_1 = (_e402 * _e403);
            let _e406: f32 = fog;
            if (_e406 > 0f) {
                {
                    let _e409: vec3<f32> = here_1;
                    let _e410: f32 = fog;
                    let _e417: vec3<f32> = sun_7;
                    let _e423: vec3<f32> = glow_5;
                    here_1 = (_e409 + (_e410 * ((u.fogC.xyz + (u.rainC.w * _e417)) + (u.fogC.w * _e423))));
                }
            }
            let _e428: vec3<f32> = acc_2;
            let _e429: f32 = through;
            let _e430: vec3<f32> = here_1;
            acc_2 = (_e428 + (_e429 * _e430));
            let _e433: f32 = through;
            let _e434: f32 = fog;
            let _e436: f32 = ds_1;
            through = (_e433 * exp((-(_e434) * _e436)));
        }
        continuing {
            let _e327: i32 = i_16;
            i_16 = (_e327 + 1i);
        }
    }
    let _e440: vec3<f32> = acc_2;
    let _e441: f32 = ds_1;
    let _e442: vec3<f32> = (_e440 * _e441);
    let _e443: f32 = through;
    return vec4<f32>(_e442.x, _e442.y, _e442.z, _e443);
}

fn walkSkipsPlant(v_69: vec4<u32>, trans_4: vec3<f32>) -> bool {
    var v_70: vec4<u32>;
    var trans_5: vec3<f32>;

    v_70 = v_69;
    trans_5 = trans_4;
    let _e255: vec4<u32> = v_70;
    let _e256: bool = plantHidden(_e255);
    let _e257: vec3<f32> = trans_5;
    let _e264: i32 = wEntry;
    return ((_e256 && all((_e257 == vec3(1f)))) && (!(LIT) || (_e264 < 0i)));
}

fn worldCol(sx_1: i32) -> i32 {
    var sx_2: i32;
    var rw: i32;
    var v_71: i32;
    var local_143: i32;

    sx_2 = sx_1;
    let _e253: i32 = W;
    let _e254: i32 = S;
    rw = (_e253 * _e254);
    let _e257: i32 = sx_2;
    let _e262: i32 = rw;
    v_71 = ((_e257 + u.view.x) % _e262);
    let _e265: i32 = v_71;
    if (_e265 < 0i) {
        let _e268: i32 = v_71;
        let _e269: i32 = rw;
        local_143 = (_e268 + _e269);
    } else {
        let _e271: i32 = v_71;
        local_143 = _e271;
    }
    let _e273: i32 = local_143;
    return _e273;
}

fn highlighted(x_97: i32, y_86: i32, z_94: i32) -> bool {
    var x_98: i32;
    var y_87: i32;
    var z_95: i32;
    var dxh: i32;
    var key_1: u32;
    var i_17: i32 = 0i;

    x_98 = x_97;
    y_87 = y_86;
    z_95 = z_94;
    if (u.view.y == 0i) {
        {
            return false;
        }
    }
    let _e263: i32 = x_98;
    let _e268: i32 = W;
    let _e269: i32 = wrapMod((_e263 - u.hiMin.x), _e268);
    dxh = _e269;
    let _e271: i32 = dxh;
    let _e276: i32 = y_87;
    let _e282: i32 = y_87;
    let _e288: i32 = z_95;
    let _e294: i32 = z_95;
    if (((((_e271 > u.hiMax.x) || (_e276 < u.hiMin.y)) || (_e282 > u.hiMax.y)) || (_e288 < u.hiMin.z)) || (_e294 > u.hiMax.z)) {
        {
            return false;
        }
    }
    if (u.view.y < 0i) {
        {
            return true;
        }
    }
    let _e307: i32 = x_98;
    let _e308: i32 = wrapX(_e307);
    let _e310: i32 = y_87;
    let _e316: i32 = z_95;
    key_1 = ((u32(_e308) | (u32(_e310) << 12u)) | (u32(_e316) << 22u));
    loop {
        let _e325: i32 = i_17;
        if !((_e325 < u.view.y)) {
            break;
        }
        {
            let _e334: i32 = i_17;
            let _e337: i32 = i_17;
            let _e344: u32 = u.hiCells[(_e337 >> 2u)][(_e334 & 3i)];
            let _e345: u32 = key_1;
            if (_e344 == _e345) {
                {
                    return true;
                }
            }
        }
        continuing {
            let _e331: i32 = i_17;
            i_17 = (_e331 + 1i);
        }
    }
    return false;
}

fn hiTint(c_57: vec3<f32>) -> vec3<f32> {
    var c_58: vec3<f32>;

    c_58 = c_57;
    let _e253: vec3<f32> = c_58;
    return mix(_e253, vec3(1f), vec3(u.hiK.x));
}

fn vineDraw(vv: vec4<u32>, x_99: i32, level_5: i32, zc: i32, z_96: i32, dx_46: i32, r_28: i32, acc_3: ptr<function, vec3<f32>>, trans_6: ptr<function, vec3<f32>>) -> f32 {
    var vv_1: vec4<u32>;
    var x_100: i32;
    var level_6: i32;
    var zc_1: i32;
    var z_97: i32;
    var dx_47: i32;
    var r_29: i32;
    var dy_32: i32;
    var t_41: vec4<f32>;
    var en: bool;
    var local_144: i32;
    var ent: i32;
    var o_4: vec4<u32>;
    var s_3: f32;
    var local_145: f32;
    var cn: f32;
    var p_45: vec4<f32> = vec4(0f);
    var co: f32 = 0f;
    var local_146: f32;
    var cover_3: f32;
    var h_16: f32;
    var e_11: vec3<f32> = vec3(0f);
    var em: bool = false;
    var wn: f32;
    var on_5: f32 = 1f;
    var col_7: vec3<f32>;
    var wp: f32;
    var pig: vec3<f32>;
    var local_147: vec3<f32>;

    vv_1 = vv;
    x_100 = x_99;
    level_6 = level_5;
    zc_1 = zc;
    z_97 = z_96;
    dx_47 = dx_46;
    r_29 = r_28;
    let _e267: i32 = S;
    let _e270: i32 = r_29;
    dy_32 = ((_e267 - 1i) - _e270);
    let _e273: vec4<u32> = vv_1;
    let _e274: i32 = x_100;
    let _e275: i32 = level_6;
    let _e276: i32 = zc_1;
    let _e277: i32 = dx_47;
    let _e278: i32 = dy_32;
    let _e279: vec4<f32> = vineTexel(_e273, _e274, _e275, _e276, _e277, _e278);
    t_41 = _e279;
    let _e281: vec4<f32> = t_41;
    let _e282: bool = vineEmits(_e281);
    en = (LIT && _e282);
    let _e285: bool = en;
    if (_e285 || ((u32(u.view.w) & EMIT_VINES) != 0u)) {
        let _e294: i32 = x_100;
        let _e295: i32 = level_6;
        let _e296: i32 = zc_1;
        let _e297: i32 = cellEntry(_e294, _e295, _e296);
        local_144 = _e297;
    } else {
        local_144 = -1i;
    }
    let _e301: i32 = local_144;
    ent = _e301;
    let _e303: vec4<u32> = vv_1;
    o_4 = _e303;
    let _e305: i32 = ent;
    let _e308: f32 = entryVineFrom(_e305, (&o_4));
    s_3 = _e308;
    let _e310: vec4<f32> = t_41;
    if (_e310.w >= 0.5f) {
        let _e315: f32 = s_3;
        local_145 = (1f - _e315);
    } else {
        local_145 = 0f;
    }
    let _e319: f32 = local_145;
    cn = _e319;
    let _e326: f32 = s_3;
    if (_e326 > 0f) {
        {
            let _e329: vec4<u32> = o_4;
            let _e330: i32 = x_100;
            let _e331: i32 = level_6;
            let _e332: i32 = zc_1;
            let _e333: i32 = dx_47;
            let _e334: i32 = dy_32;
            let _e335: vec4<f32> = vineTexel(_e329, _e330, _e331, _e332, _e333, _e334);
            p_45 = _e335;
            let _e336: vec4<f32> = p_45;
            if (_e336.w >= 0.5f) {
                let _e340: f32 = s_3;
                local_146 = _e340;
            } else {
                local_146 = 0f;
            }
            let _e343: f32 = local_146;
            co = _e343;
        }
    }
    let _e344: f32 = cn;
    let _e345: f32 = co;
    cover_3 = (_e344 + _e345);
    let _e348: f32 = cover_3;
    if (_e348 <= 0f) {
        {
            return 0f;
        }
    }
    let _e352: i32 = z_97;
    let _e354: f32 = hazeAt(f32(_e352));
    h_16 = _e354;
    let _e361: f32 = cn;
    wn = _e361;
    let _e363: f32 = cn;
    let _e366: bool = en;
    if ((_e363 > 0f) && _e366) {
        {
            let _e368: vec4<f32> = t_41;
            e_11 = _e368.xyz;
            let _e372: i32 = ent;
            let _e377: bool = entryEmit(_e372, (&e_11), (&on_5));
            let _e378: f32 = cn;
            let _e380: f32 = on_5;
            wn = (_e378 * (1f - _e380));
            let _e383: vec3<f32> = e_11;
            em = any((_e383 > vec3(0f)));
        }
    }
    let _e388: f32 = cn;
    let _e389: vec3<f32> = e_11;
    col_7 = (_e388 * _e389);
    let _e392: f32 = wn;
    let _e393: f32 = co;
    wp = (_e392 + _e393);
    let _e396: f32 = wp;
    if (_e396 > 0f) {
        {
            let _e399: f32 = wn;
            let _e400: vec4<f32> = t_41;
            let _e403: f32 = co;
            let _e404: vec4<f32> = p_45;
            let _e408: f32 = wp;
            pig = (((_e399 * _e400.xyz) + (_e403 * _e404.xyz)) / vec3(_e408));
            if LIT {
                {
                    let _e412: vec3<f32> = pig;
                    let _e413: i32 = x_100;
                    let _e414: i32 = level_6;
                    let _e415: i32 = z_97;
                    let _e416: i32 = dx_47;
                    let _e417: i32 = dy_32;
                    let _e418: i32 = x_100;
                    let _e419: i32 = z_97;
                    let _e422: i32 = level_6;
                    let _e423: f32 = canopyAt(_e418, (_e419 - 1i), _e422);
                    let _e424: vec3<f32> = shadeFront(_e412, _e413, _e414, _e415, _e416, _e417, _e423);
                    pig = _e424;
                }
            }
            let _e425: vec3<f32> = col_7;
            let _e426: f32 = wp;
            let _e427: vec3<f32> = pig;
            col_7 = (_e425 + (_e426 * _e427));
        }
    }
    let _e430: f32 = cover_3;
    if (_e430 >= 0.999f) {
        {
            let _e433: vec3<f32> = (*acc_3);
            let _e434: vec3<f32> = (*trans_6);
            let _e435: vec3<f32> = col_7;
            let _e436: f32 = h_16;
            let _e437: vec3<f32> = hazed(_e435, _e436);
            (*acc_3) = (_e433 + (_e434 * _e437));
            let _e440: bool = em;
            if _e440 {
                let _e441: vec3<f32> = (*trans_6);
                let _e442: vec3<f32> = e_11;
                let _e443: f32 = cn;
                let _e445: f32 = h_16;
                let _e446: vec3<f32> = hazed((_e442 * _e443), _e445);
                let _e447: f32 = h_16;
                let _e448: vec3<f32> = emitterTrans(_e441, _e446, _e447);
                local_147 = _e448;
            } else {
                local_147 = vec3(0f);
            }
            let _e452: vec3<f32> = local_147;
            (*trans_6) = _e452;
            return 1f;
        }
    }
    let _e454: vec3<f32> = (*acc_3);
    let _e455: vec3<f32> = (*trans_6);
    let _e456: f32 = cover_3;
    let _e458: vec3<f32> = col_7;
    let _e459: f32 = cover_3;
    let _e462: f32 = h_16;
    let _e463: vec3<f32> = hazed((_e458 / vec3(_e459)), _e462);
    (*acc_3) = (_e454 + ((_e455 * _e456) * _e463));
    let _e466: vec3<f32> = (*trans_6);
    let _e468: f32 = cover_3;
    (*trans_6) = (_e466 * (1f - _e468));
    let _e471: f32 = cover_3;
    return _e471;
}

fn main_1() {
    var px_15: vec2<i32>;
    var wcol: i32;
    var x_101: i32;
    var dx_48: i32;
    var acc_4: vec3<f32> = vec3(0f);
    var trans_7: vec3<f32> = vec3(1f);
    var z_98: i32 = 0i;
    var zHit: f32 = FAR;
    var plantBehind: bool = false;
    var q_20: i32;
    var level_7: i32;
    var r_30: i32;
    var v_72: vec4<u32>;
    var solid_1: bool;
    var vc: vec4<u32>;
    var vz: i32;
    var vineHere: bool;
    var k_22: i32;
    var q_21: i32;
    var p_46: i32;
    var fy: i32;
    var fl_4: i32;
    var found_3: bool;
    var art: vec3<f32>;
    var glow_6: vec3<f32>;
    var local_148: vec3<f32>;
    var below: i32;
    var v_73: vec4<u32>;
    var p_47: i32;
    var cap_3: vec3<f32>;
    var glow_7: vec3<f32>;
    var local_149: i32;
    var cl: i32;
    var local_150: vec3<f32>;
    var emitted: vec3<f32> = vec3(0f);
    var sky_2: vec3<f32>;
    var local_151: i32;
    var zEnd_13: i32;
    var local_152: f32;
    var local_153: f32;
    var local_154: vec3<f32>;
    var colour_4: vec3<f32>;
    var sc_1: vec4<f32>;

    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;
    let _e270: vec4<f32> = gl_FragCoord_1;
    px_15 = vec2<i32>(_e270.xy);
    if WEATHER {
        {
            let _e274: vec2<i32> = px_15;
            let _e276: i32 = worldCol(_e274.x);
            PX = _e276;
            let _e277: vec2<i32> = px_15;
            PY = _e277.y;
            let _e279: vec2<i32> = px_15;
            let _e280: vec4<f32> = weatherOver(_e279);
            outColour = _e280;
            return;
        }
    }
    if SKY {
        {
            let _e281: vec2<i32> = px_15;
            let _e283: vec2<i32> = px_15;
            let _e285: vec4<f32> = litSky(_e281.x, _e283.y);
            outColour = _e285;
            return;
        }
    }
    let _e286: vec2<i32> = px_15;
    let _e288: i32 = worldCol(_e286.x);
    wcol = _e288;
    let _e290: i32 = wcol;
    let _e291: i32 = S;
    x_101 = (_e290 / _e291);
    let _e294: i32 = wcol;
    let _e295: i32 = x_101;
    let _e296: i32 = S;
    dx_48 = (_e294 - (_e295 * _e296));
    if LIT {
        {
            let _e306: i32 = wcol;
            PX = _e306;
            let _e307: vec2<i32> = px_15;
            PY = _e307.y;
            waterLitReset();
        }
    }
    loop {
        let _e314: i32 = z_98;
        let _e315: i32 = D;
        if !((_e314 < _e315)) {
            break;
        }
        {
            let _e321: i32 = BASE;
            let _e322: i32 = z_98;
            let _e323: i32 = RISE;
            let _e326: vec2<i32> = px_15;
            q_20 = (((_e321 - (_e322 * _e323)) - _e326.y) - 1i);
            let _e332: i32 = q_20;
            if (_e332 < 0i) {
                {
                    break;
                }
            }
            let _e335: i32 = q_20;
            let _e336: i32 = S;
            level_7 = (_e335 / _e336);
            let _e339: i32 = q_20;
            let _e340: i32 = level_7;
            let _e341: i32 = S;
            r_30 = (_e339 - (_e340 * _e341));
            let _e345: i32 = level_7;
            let _e346: i32 = H;
            if (_e345 < _e346) {
                {
                    let _e348: i32 = x_101;
                    let _e349: i32 = level_7;
                    let _e350: i32 = z_98;
                    let _e351: vec4<u32> = at(_e348, _e349, _e350);
                    v_72 = _e351;
                    let _e353: vec4<u32> = v_72;
                    let _e354: bool = solidV(_e353);
                    solid_1 = _e354;
                    let _e356: vec4<u32> = v_72;
                    vc = _e356;
                    let _e358: i32 = z_98;
                    vz = _e358;
                    vineHere = false;
                    let _e362: bool = solid_1;
                    if _e362 {
                        {
                            let _e363: i32 = z_98;
                            if ((_e363 > 0i) && (u.tex.y != 0i)) {
                                {
                                    let _e372: i32 = x_101;
                                    let _e373: i32 = level_7;
                                    let _e374: i32 = z_98;
                                    let _e377: vec4<u32> = at(_e372, _e373, (_e374 - 1i));
                                    vc = _e377;
                                    let _e378: i32 = z_98;
                                    vz = (_e378 - 1i);
                                    let _e381: vec4<u32> = vc;
                                    let _e382: bool = isVine(_e381);
                                    let _e383: vec4<u32> = vc;
                                    let _e384: i32 = vineKind(_e383);
                                    vineHere = (_e382 && (_e384 == VINE_FLUSH));
                                }
                            }
                        }
                    } else {
                        {
                            let _e387: vec4<u32> = v_72;
                            if (_e387.y != 0u) {
                                {
                                    let _e391: i32 = x_101;
                                    let _e392: i32 = level_7;
                                    let _e393: i32 = z_98;
                                    let _e394: vec4<u32> = v_72;
                                    let _e395: vec2<i32> = px_15;
                                    waterAt(_e391, _e392, _e393, _e394, _e395.y, (&acc_4), (&trans_7));
                                }
                            }
                            let _e401: vec4<u32> = v_72;
                            let _e402: bool = isVine(_e401);
                            if _e402 {
                                {
                                    let _e403: vec4<u32> = v_72;
                                    let _e404: i32 = vineKind(_e403);
                                    k_22 = _e404;
                                    let _e407: i32 = S;
                                    q_21 = max(1i, (_e407 / 4i));
                                    let _e412: i32 = k_22;
                                    let _e414: i32 = k_22;
                                    let _e416: i32 = dx_48;
                                    let _e417: i32 = q_21;
                                    let _e421: i32 = k_22;
                                    let _e423: i32 = dx_48;
                                    let _e424: i32 = S;
                                    let _e425: i32 = q_21;
                                    vineHere = (((_e412 == VINE_CURTAIN) || ((_e414 == VINE_SLIVER_L) && (_e416 < _e417))) || ((_e421 == VINE_SLIVER_R) && (_e423 >= (_e424 - _e425))));
                                }
                            }
                        }
                    }
                    let _e430: bool = vineHere;
                    // vineDraw changes acc/trans and must only run for actual vines.
                    if _e430 {
                        let _e431: vec4<u32> = vc;
                        let _e432: i32 = x_101;
                        let _e433: i32 = level_7;
                        let _e434: i32 = vz;
                        let _e435: i32 = z_98;
                        let _e436: i32 = dx_48;
                        let _e437: i32 = r_30;
                        let _e442: f32 = vineDraw(_e431, _e432, _e433, _e434, _e435, _e436, _e437, (&acc_4), (&trans_7));
                        if (_e442 >= 1f) {
                            {
                                let _e446: i32 = z_98;
                                zHit = f32(_e446);
                                break;
                            }
                        }
                    }
                    let _e448: bool = solid_1;
                    if _e448 {
                        {
                            let _e449: vec3<f32> = acc_4;
                            let _e450: vec3<f32> = trans_7;
                            let _e451: i32 = x_101;
                            let _e452: i32 = level_7;
                            let _e453: i32 = z_98;
                            let _e454: vec4<u32> = v_72;
                            let _e455: i32 = r_30;
                            let _e456: i32 = dx_48;
                            let _e457: vec3<f32> = blockFront(_e451, _e452, _e453, _e454, _e455, _e456);
                            acc_4 = (_e449 + (_e450 * _e457));
                            trans_7 = vec3(0f);
                            let _e462: i32 = z_98;
                            zHit = f32(_e462);
                            break;
                        }
                    }
                    let _e464: vec4<u32> = v_72;
                    let _e465: i32 = partOf(_e464);
                    p_46 = _e465;
                    let _e467: i32 = p_46;
                    let _e470: vec4<u32> = v_72;
                    let _e471: vec3<f32> = trans_7;
                    let _e472: bool = walkSkipsPlant(_e470, _e471);
                    if ((_e467 != 0i) && !(_e472)) {
                        {
                            let _e475: i32 = level_7;
                            fy = _e475;
                            let _e477: i32 = S;
                            let _e480: i32 = r_30;
                            fl_4 = ((_e477 - 1i) - _e480);
                            found_3 = true;
                            let _e485: vec4<u32> = v_72;
                            let _e486: bool = isPosed(_e485);
                            if _e486 {
                                {
                                    let _e487: i32 = x_101;
                                    let _e488: i32 = level_7;
                                    let _e489: i32 = z_98;
                                    let _e490: i32 = r_30;
                                    let _e495: bool = posedFind(_e487, _e488, _e489, _e490, (&fy), (&fl_4));
                                    found_3 = _e495;
                                    let _e496: i32 = x_101;
                                    let _e497: i32 = fy;
                                    let _e498: i32 = z_98;
                                    let _e499: vec4<u32> = at(_e496, _e497, _e498);
                                    v_72 = _e499;
                                }
                            }
                            let _e502: bool = found_3;
                            let _e503: i32 = x_101;
                            let _e504: i32 = fy;
                            let _e505: i32 = z_98;
                            let _e506: vec4<u32> = v_72;
                            let _e507: i32 = fl_4;
                            let _e508: i32 = dx_48;
                            var _e513: bool = false;
                            if _e502 {
                                _e513 = glyphFrontAt(_e503, _e504, _e505, _e506, _e507, _e508, (&art), (&glow_6));
                            }
                            if (_e502 && _e513) {
                                {
                                    let _e515: vec4<u32> = v_72;
                                    let _e516: bool = plantHidden(_e515);
                                    plantBehind = _e516;
                                    let _e517: vec4<u32> = v_72;
                                    let _e518: i32 = partOf(_e517);
                                    let _e520: i32 = x_101;
                                    let _e521: i32 = fy;
                                    let _e522: i32 = z_98;
                                    let _e523: bool = highlighted(_e520, _e521, _e522);
                                    if ((_e518 == ANIMAL_INTERIM) && _e523) {
                                        {
                                            let _e525: vec3<f32> = art;
                                            let _e526: vec3<f32> = hiTint(_e525);
                                            art = _e526;
                                        }
                                    }
                                    let _e527: vec3<f32> = acc_4;
                                    let _e528: vec3<f32> = trans_7;
                                    let _e529: vec3<f32> = art;
                                    acc_4 = (_e527 + (_e528 * _e529));
                                    let _e532: vec3<f32> = glow_6;
                                    if (LIT && any((_e532 > vec3(0f)))) {
                                        let _e538: vec3<f32> = trans_7;
                                        let _e539: vec3<f32> = glow_6;
                                        let _e540: i32 = z_98;
                                        let _e542: f32 = hazeAt(f32(_e540));
                                        let _e543: vec3<f32> = emitterTrans(_e538, _e539, _e542);
                                        local_148 = _e543;
                                    } else {
                                        local_148 = vec3(0f);
                                    }
                                    let _e547: vec3<f32> = local_148;
                                    trans_7 = _e547;
                                    let _e548: i32 = z_98;
                                    zHit = f32(_e548);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            let _e550: i32 = level_7;
            below = (_e550 - 1i);
            let _e554: i32 = r_30;
            let _e555: i32 = RISE;
            let _e557: i32 = below;
            let _e561: i32 = below;
            let _e562: i32 = H;
            if (((_e554 < _e555) && (_e557 >= 0i)) && (_e561 < _e562)) {
                {
                    let _e565: i32 = x_101;
                    let _e566: i32 = below;
                    let _e567: i32 = z_98;
                    let _e568: vec4<u32> = at(_e565, _e566, _e567);
                    v_73 = _e568;
                    let _e570: vec4<u32> = v_73;
                    let _e571: bool = solidV(_e570);
                    if _e571 {
                        {
                            let _e572: vec3<f32> = acc_4;
                            let _e573: vec3<f32> = trans_7;
                            let _e574: i32 = x_101;
                            let _e575: i32 = below;
                            let _e576: i32 = z_98;
                            let _e577: vec4<u32> = v_73;
                            let _e578: i32 = r_30;
                            let _e579: i32 = dx_48;
                            let _e580: vec3<f32> = blockTop(_e574, _e575, _e576, _e577, _e578, _e579);
                            acc_4 = (_e572 + (_e573 * _e580));
                            trans_7 = vec3(0f);
                            let _e585: i32 = z_98;
                            let _e587: i32 = r_30;
                            let _e591: i32 = RISE;
                            zHit = (f32(_e585) + ((f32(_e587) + 0.5f) / f32(_e591)));
                            break;
                        }
                    }
                    let _e595: vec4<u32> = v_73;
                    if (_e595.y != 0u) {
                        {
                            let _e599: i32 = x_101;
                            let _e600: i32 = below;
                            let _e601: i32 = z_98;
                            let _e602: vec4<u32> = v_73;
                            let _e603: vec2<i32> = px_15;
                            waterAt(_e599, _e600, _e601, _e602, _e603.y, (&acc_4), (&trans_7));
                        }
                    }
                    let _e609: vec4<u32> = v_73;
                    let _e610: i32 = partOf(_e609);
                    p_47 = _e610;
                    let _e614: vec4<u32> = v_73;
                    let _e615: bool = isPosed(_e614);
                    if _e615 {
                        let _e616: vec4<u32> = v_73;
                        let _e617: i32 = level_7;
                        let _e618: i32 = r_30;
                        let _e619: i32 = posedCapRow(_e616, _e617, _e618);
                        local_149 = _e619;
                    } else {
                        let _e620: i32 = RISE;
                        let _e623: i32 = r_30;
                        local_149 = ((_e620 - 1i) - _e623);
                    }
                    let _e626: i32 = local_149;
                    cl = _e626;
                    let _e628: i32 = p_47;
                    let _e629: bool = isBlockPart(_e628);
                    let _e630: vec4<u32> = v_73;
                    let _e631: vec3<f32> = trans_7;
                    let _e632: bool = walkSkipsPlant(_e630, _e631);
                    let _e635: i32 = cl;
                    let _e639: i32 = x_101;
                    let _e640: i32 = below;
                    let _e641: i32 = z_98;
                    let _e642: vec4<u32> = v_73;
                    let _e643: i32 = cl;
                    let _e644: i32 = dx_48;
                    var _e649: bool = false;
                    if ((_e629 && !(_e632)) && (_e635 >= 0i)) {
                        _e649 = glyphCapAt(_e639, _e640, _e641, _e642, _e643, _e644, (&cap_3), (&glow_7));
                    }
                    if (((_e629 && !(_e632)) && (_e635 >= 0i)) && _e649) {
                        {
                            let _e651: vec4<u32> = v_73;
                            let _e652: bool = plantHidden(_e651);
                            plantBehind = _e652;
                            let _e653: i32 = p_47;
                            let _e655: i32 = x_101;
                            let _e656: i32 = below;
                            let _e657: i32 = z_98;
                            let _e658: bool = highlighted(_e655, _e656, _e657);
                            if ((_e653 == ANIMAL_INTERIM) && _e658) {
                                {
                                    let _e660: vec3<f32> = cap_3;
                                    let _e661: vec3<f32> = hiTint(_e660);
                                    cap_3 = _e661;
                                }
                            }
                            let _e662: vec3<f32> = acc_4;
                            let _e663: vec3<f32> = trans_7;
                            let _e664: vec3<f32> = cap_3;
                            acc_4 = (_e662 + (_e663 * _e664));
                            let _e667: vec3<f32> = glow_7;
                            if (LIT && any((_e667 > vec3(0f)))) {
                                let _e673: vec3<f32> = trans_7;
                                let _e674: vec3<f32> = glow_7;
                                let _e675: i32 = z_98;
                                let _e677: i32 = r_30;
                                let _e679: i32 = RISE;
                                let _e683: f32 = hazeAt((f32(_e675) + (f32(_e677) / f32(_e679))));
                                let _e684: vec3<f32> = emitterTrans(_e673, _e674, _e683);
                                local_150 = _e684;
                            } else {
                                local_150 = vec3(0f);
                            }
                            let _e688: vec3<f32> = local_150;
                            trans_7 = _e688;
                            let _e689: i32 = z_98;
                            let _e691: i32 = r_30;
                            let _e695: i32 = RISE;
                            zHit = (f32(_e689) + ((f32(_e691) + 0.5f) / f32(_e695)));
                            break;
                        }
                    }
                }
            }
            let _e699: vec3<f32> = trans_7;
            if all((_e699 <= vec3(0f))) {
                {
                    let _e704: i32 = z_98;
                    zHit = f32(_e704);
                    break;
                }
            }
        }
        continuing {
            let _e318: i32 = z_98;
            z_98 = (_e318 + 1i);
        }
    }
    let _e709: vec3<f32> = trans_7;
    if (LIT && (_e709.x < -0.5f)) {
        {
            let _e715: vec3<f32> = trans_7;
            emitted = (-(_e715) - vec3(1f));
            trans_7 = vec3(0f);
        }
    }
    let _e722: vec2<i32> = px_15;
    let _e724: vec2<i32> = px_15;
    let _e726: vec3<f32> = skyColor(_e722.x, _e724.y);
    sky_2 = _e726;
    if (LIT && (u.dayB.w > 0f)) {
        {
            let _e734: vec3<f32> = emitted;
            let _e735: vec3<f32> = trans_7;
            let _e736: vec3<f32> = sky_2;
            let _e738: vec2<i32> = px_15;
            let _e740: vec4<f32> = textureLoad(wgpu_backdropTex, _e738, 0i);
            emitted = (_e734 + ((_e735 * _e736) * _e740.w));
        }
    }
    let _e744: i32 = wEntry;
    if (LIT && (_e744 >= 0i)) {
        let _e748: i32 = z_98;
        let _e749: i32 = wEntry;
        local_151 = min(_e748, ((_e749 >> 12u) & 4095i));
    } else {
        let _e756: i32 = z_98;
        local_151 = _e756;
    }
    let _e758: i32 = local_151;
    zEnd_13 = _e758;
    let _e760: vec3<f32> = emitted;
    if WEATHER_DEPTH {
        let _e761: i32 = zEnd_13;
        local_152 = (f32(min(_e761, 255i)) / 255f);
    } else {
        local_152 = 1f;
    }
    let _e769: f32 = local_152;
    emitOut = vec4<f32>(_e760.x, _e760.y, _e760.z, _e769);
    let _e774: bool = plantBehind;
    if _e774 {
        let _e775: f32 = zHit;
        local_153 = (_e775 - 1f);
    } else {
        let _e778: f32 = zHit;
        local_153 = _e778;
    }
    let _e780: f32 = local_153;
    depthOut = _e780;
    if LIT {
        let _e781: vec3<f32> = acc_4;
        local_154 = _e781;
    } else {
        let _e782: vec3<f32> = acc_4;
        local_154 = (_e782 * u.tint.xyz);
    }
    let _e788: vec3<f32> = local_154;
    let _e789: vec3<f32> = trans_7;
    let _e790: vec3<f32> = sky_2;
    colour_4 = (_e788 + (_e789 * _e790));
    let _e794: i32 = wEntry;
    if (LIT && (_e794 >= 0i)) {
        {
            let _e798: vec3<f32> = colour_4;
            let _e799: vec3<f32> = litWaterFinish(_e798);
            colour_4 = _e799;
        }
    }
    if (LIT && VOLUMETRIC) {
        {
            let _e801: i32 = zEnd_13;
            let _e802: vec4<f32> = inScatter(_e801);
            sc_1 = _e802;
            let _e804: vec3<f32> = colour_4;
            let _e805: vec4<f32> = sc_1;
            let _e808: vec4<f32> = sc_1;
            colour_4 = ((_e804 * _e805.w) + _e808.xyz);
        }
    }
    let _e811: vec3<f32> = colour_4;
    outColour = vec4<f32>(_e811.x, _e811.y, _e811.z, 1f);
    if (!(LIT) && (((u.rainK.x + u.rainK.y) + u.rainK.z) > 0f)) {
        {
            let _e832: vec4<f32> = outColour;
            let _e834: vec2<i32> = px_15;
            let _e836: vec2<i32> = px_15;
            let _e838: vec4<f32> = outColour;
            let _e840: vec3<f32> = flatRain(_e834.x, _e836.y, _e838.xyz);
            outColour.x = _e840.x;
            outColour.y = _e840.y;
            outColour.z = _e840.z;
            return;
        }
    } else {
        return;
    }
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e383: vec4<f32> = outColour;
    let _e385: vec4<f32> = emitOut;
    let _e387: f32 = depthOut;
    return FragmentOutput(_e383, _e385, _e387);
}
