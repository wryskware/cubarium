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

struct FigurePush {
    k: vec4<i32>,
    atlas: vec4<f32>,
}

struct Tile {
    pos: vec4<f32>,
    ax: vec4<f32>,
    ay: vec4<f32>,
    az: vec4<f32>,
    rect: vec4<i32>,
    orig: vec4<i32>,
    depth: vec4<f32>,
    faceSize: vec4<i32>,
    rig: vec4<i32>,
}

struct Fig {
    anchor: vec4<f32>,
    tiles: vec4<i32>,
    k: vec4<f32>,
}

struct FigureVoxels {
    vox: array<vec4<u32>>,
}

struct FigureTiles {
    tiles: array<Tile>,
}

struct FigureFigs {
    figs: array<Fig>,
}

struct FigureBones {
    bones: array<vec4<f32>>,
}

struct Pal {
    base: vec4<f32>,
    emit: vec4<f32>,
}

struct FigurePalette {
    pal: array<Pal>,
}

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
    @location(1) outEmit: vec4<f32>,
    @location(2) outZ: f32,
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
const FACE_O: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, 0f), vec3<f32>(1f, 0f, 1f));
const FACE_U: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(-1f, 0f, 0f));
const FACE_V: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f));
const FACE_N: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(-1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, -1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f));

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
var<immediate> pc: FigurePush;
@group(1) @binding(0) 
var<storage> global_1: FigureVoxels;
@group(1) @binding(1) 
var<storage> global_2: FigureTiles;
@group(1) @binding(2) 
var<storage> global_3: FigureFigs;
@group(1) @binding(4) 
var<storage> global_4: FigureBones;
@group(1) @binding(3) 
var<storage> global_5: FigurePalette;
var<private> vWorld_1: vec3<f32>;
var<private> vNormal_1: vec3<f32>;
var<private> vFace_1: vec2<f32>;
var<private> vInfo_1: vec4<i32>;
var<private> vFaceSize_1: vec2<i32>;
var<private> vRimWorld_1: vec3<f32>;
var<private> vRimFace_1: vec2<f32>;
var<private> vCapSize_1: vec2<i32>;
var<private> outColour: vec4<f32>;
var<private> outEmit: vec4<f32>;
var<private> outZ: f32;

fn wrapMod(a: i32, m: i32) -> i32 {
    var a_1: i32;
    var m_1: i32;
    var local: i32;

    a_1 = a;
    m_1 = m;
    let _e166: i32 = a_1;
    if (_e166 >= 0i) {
        let _e169: i32 = a_1;
        let _e170: i32 = m_1;
        local = (_e169 % _e170);
    } else {
        let _e172: i32 = m_1;
        let _e177: i32 = a_1;
        let _e179: i32 = m_1;
        local = ((_e172 - 1i) - ((-1i - _e177) % _e179));
    }
    let _e183: i32 = local;
    return _e183;
}

fn wrapX(x: i32) -> i32 {
    var x_1: i32;

    x_1 = x;
    let _e164: i32 = x_1;
    let _e165: i32 = W;
    let _e166: i32 = wrapMod(_e164, _e165);
    return _e166;
}

fn inY(y: i32) -> bool {
    var y_1: i32;

    y_1 = y;
    let _e164: i32 = y_1;
    let _e167: i32 = y_1;
    let _e168: i32 = H;
    return ((_e164 >= 0i) && (_e167 < _e168));
}

fn at(x_2: i32, y_2: i32, z: i32) -> vec4<u32> {
    var x_3: i32;
    var y_3: i32;
    var z_1: i32;

    x_3 = x_2;
    y_3 = y_2;
    z_1 = z;
    let _e168: i32 = x_3;
    let _e169: i32 = wrapX(_e168);
    let _e170: i32 = y_3;
    let _e171: i32 = z_1;
    let _e174: vec4<u32> = textureLoad(wgpu_voxels, vec3<i32>(_e169, _e170, _e171), 0i);
    return _e174;
}

fn matOf(v: vec4<u32>) -> i32 {
    var v_1: vec4<u32>;

    v_1 = v;
    let _e164: vec4<u32> = v_1;
    return i32((_e164.x & 3u));
}

fn partOf(v_2: vec4<u32>) -> i32 {
    var v_3: vec4<u32>;

    v_3 = v_2;
    let _e164: vec4<u32> = v_3;
    return i32(((_e164.x >> 2u) & 7u));
}

fn glyphOf(v_4: vec4<u32>) -> i32 {
    var v_5: vec4<u32>;

    v_5 = v_4;
    let _e164: vec4<u32> = v_5;
    return i32((_e164.x >> 5u));
}

fn isBlockPart(p: i32) -> bool {
    var p_1: i32;

    p_1 = p;
    let _e164: i32 = p_1;
    let _e166: i32 = p_1;
    let _e169: i32 = p_1;
    let _e172: i32 = p_1;
    let _e175: i32 = p_1;
    return (((((_e164 == TRUNK) || (_e166 == CROWN)) || (_e169 == CROWN_HEART)) || (_e172 == ANIMAL_INTERIM)) || (_e175 == LOG));
}

fn plantBlockPart(p_2: i32) -> bool {
    var p_3: i32;

    p_3 = p_2;
    let _e164: i32 = p_3;
    let _e166: i32 = p_3;
    let _e169: i32 = p_3;
    return (((_e164 == TRUNK) || (_e166 == CROWN)) || (_e169 == CROWN_HEART));
}

fn softMark(v_6: vec4<u32>) -> bool {
    var v_7: vec4<u32>;

    v_7 = v_6;
    let _e164: vec4<u32> = v_7;
    let _e165: i32 = partOf(_e164);
    let _e168: vec4<u32> = v_7;
    let _e169: i32 = glyphOf(_e168);
    let _e173: vec4<u32> = v_7;
    let _e178: vec4<u32> = v_7;
    return ((((POSE && (_e165 == ANIMAL_INTERIM)) && (_e169 == 0i)) && (_e173.z != 0u)) && ((_e178.z >> 6u) < 2u));
}

fn solidV(v_8: vec4<u32>) -> bool {
    var v_9: vec4<u32>;

    v_9 = v_8;
    let _e164: vec4<u32> = v_9;
    let _e165: i32 = matOf(_e164);
    return (_e165 != 0i);
}

fn solidAt(x_4: i32, y_4: i32, z_2: i32) -> bool {
    var x_5: i32;
    var y_5: i32;
    var z_3: i32;

    x_5 = x_4;
    y_5 = y_4;
    z_3 = z_2;
    let _e168: i32 = y_5;
    let _e169: bool = inY(_e168);
    let _e170: i32 = x_5;
    let _e171: i32 = y_5;
    let _e172: i32 = z_3;
    let _e173: vec4<u32> = at(_e170, _e171, _e172);
    let _e174: bool = solidV(_e173);
    return (_e169 && _e174);
}

fn freeQ(x_6: i32, y_6: i32, z_4: i32) -> i32 {
    var x_7: i32;
    var y_7: i32;
    var z_5: i32;
    var local_1: i32;

    x_7 = x_6;
    y_7 = y_6;
    z_5 = z_4;
    let _e168: i32 = y_7;
    let _e169: bool = inY(_e168);
    if _e169 {
        let _e170: i32 = x_7;
        let _e171: i32 = y_7;
        let _e172: i32 = z_5;
        let _e173: vec4<u32> = at(_e170, _e171, _e172);
        local_1 = i32(_e173.y);
    } else {
        local_1 = 0i;
    }
    let _e178: i32 = local_1;
    return _e178;
}

fn fillPxQ(q: i32) -> i32 {
    var q_1: i32;
    var local_2: i32;

    q_1 = q;
    let _e164: i32 = q_1;
    if (_e164 == 0i) {
        local_2 = 0i;
    } else {
        let _e168: i32 = q_1;
        let _e172: i32 = S;
        let _e180: i32 = S;
        local_2 = clamp(i32(floor((((f32(_e168) / 255f) * f32(_e172)) + 0.5f))), 1i, _e180);
    }
    let _e183: i32 = local_2;
    return _e183;
}

fn fillPxAt(x_8: i32, y_8: i32, z_6: i32) -> i32 {
    var x_9: i32;
    var y_9: i32;
    var z_7: i32;

    x_9 = x_8;
    y_9 = y_8;
    z_7 = z_6;
    let _e168: i32 = x_9;
    let _e169: i32 = y_9;
    let _e170: i32 = z_7;
    let _e171: i32 = freeQ(_e168, _e169, _e170);
    let _e172: i32 = fillPxQ(_e171);
    return _e172;
}

fn waterOpenUp(x_10: i32, y_10: i32, z_8: i32) -> bool {
    var x_11: i32;
    var y_11: i32;
    var z_9: i32;

    x_11 = x_10;
    y_11 = y_10;
    z_9 = z_8;
    let _e168: i32 = x_11;
    let _e169: i32 = y_11;
    let _e170: i32 = z_9;
    let _e171: i32 = fillPxAt(_e168, _e169, _e170);
    let _e172: i32 = S;
    let _e174: i32 = x_11;
    let _e175: i32 = y_11;
    let _e178: i32 = z_9;
    let _e179: bool = solidAt(_e174, (_e175 + 1i), _e178);
    let _e181: i32 = x_11;
    let _e182: i32 = y_11;
    let _e185: i32 = z_9;
    let _e186: i32 = freeQ(_e181, (_e182 + 1i), _e185);
    return ((_e171 < _e172) || (!(_e179) && (_e186 == 0i)));
}

fn frontRow(y_12: i32, z_10: i32) -> i32 {
    var y_13: i32;
    var z_11: i32;

    y_13 = y_12;
    z_11 = z_10;
    let _e166: i32 = BASE;
    let _e167: i32 = y_13;
    let _e170: i32 = S;
    let _e173: i32 = z_11;
    let _e174: i32 = RISE;
    return ((_e166 - ((_e167 + 1i) * _e170)) - (_e173 * _e174));
}

fn texOn(slot: i32) -> bool {
    var slot_1: i32;

    slot_1 = slot;
    let _e168: i32 = slot_1;
    let _e175: i32 = slot_1;
    return ((_e168 >= 0i) && ((u.tex.x & (1i << u32(_e175))) != 0i));
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
    let _e174: i32 = x_13;
    let _e178: i32 = y_15;
    let _e183: i32 = z_13;
    let _e188: i32 = salt_1;
    h = ((((u32(_e174) * 2654435761u) ^ (u32(_e178) * 2246822519u)) ^ (u32(_e183) * 3266489917u)) ^ (u32(_e188) * 668265263u));
    let _e194: u32 = h;
    let _e195: u32 = h;
    h = (_e194 ^ (_e195 >> 15u));
    let _e200: u32 = h;
    h = (_e200 * 739982445u);
    let _e203: u32 = h;
    let _e204: u32 = h;
    h = (_e203 ^ (_e204 >> 12u));
    let _e209: u32 = h;
    return _e209;
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
    let _e174: i32 = x_15;
    let _e175: i32 = y_17;
    let _e176: i32 = z_15;
    let _e177: i32 = salt_3;
    let _e178: u32 = cellHash(_e174, _e175, _e176, _e177);
    return i32((_e178 & 3u));
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
    let _e180: i32 = x_17;
    let _e181: i32 = y_19;
    let _e182: i32 = z_17;
    let _e183: i32 = salt_5;
    let _e184: i32 = texVariant(_e180, _e181, _e182, _e183);
    v_10 = _e184;
    let _e186: i32 = v_10;
    let _e187: i32 = S;
    let _e189: i32 = dx_1;
    let _e191: i32 = slot_3;
    let _e192: i32 = S;
    let _e194: i32 = dy_1;
    let _e198: vec4<f32> = textureLoad(wgpu_faceTex, vec2<i32>(((_e186 * _e187) + _e189), ((_e191 * _e192) + _e194)), 0i);
    return _e198;
}

fn srgbToLinear(c: vec3<f32>) -> vec3<f32> {
    var c_1: vec3<f32>;

    c_1 = c;
    let _e168: vec3<f32> = c_1;
    let _e172: vec3<f32> = c_1;
    let _e184: vec3<f32> = c_1;
    return mix((_e168 / vec3(12.92f)), pow(((_e172 + vec3(0.055f)) / vec3(1.055f)), vec3(2.4f)), step(vec3(0.04045f), _e184));
}

fn isVine(v_11: vec4<u32>) -> bool {
    var v_12: vec4<u32>;

    v_12 = v_11;
    let _e177: vec4<u32> = v_12;
    let _e184: vec4<u32> = v_12;
    return (((u.tex.y != 0i) && ((_e177.x & 31u) == 0u)) && ((_e184.x >> 5u) != 0u));
}

fn vineKind(v_13: vec4<u32>) -> i32 {
    var v_14: vec4<u32>;

    v_14 = v_13;
    let _e172: vec4<u32> = v_14;
    return (i32((_e172.x >> 5u)) - 1i);
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
    let _e182: vec4<u32> = v_16;
    row = i32((_e182.w & 15u));
    let _e188: vec4<u32> = v_16;
    let _e195: vec4<u32> = v_16;
    col = ((i32((_e188.z & 15u)) * 16i) + i32((_e195.z >> 4u)));
    let _e203: i32 = col;
    let _e204: i32 = S;
    let _e206: i32 = dx_3;
    let _e208: i32 = row;
    let _e209: i32 = S;
    let _e211: i32 = dy_3;
    let _e215: vec4<f32> = textureLoad(wgpu_vineTex, vec2<i32>(((_e203 * _e204) + _e206), ((_e208 * _e209) + _e211)), 0i);
    t = _e215;
    let _e217: vec4<u32> = v_16;
    accent = i32(((_e217.w >> 4u) & 3u));
    let _e226: i32 = accent;
    if (_e226 != 0i) {
        {
            let _e229: i32 = S;
            j = (_e229 / 8i);
            let _e233: i32 = x_19;
            let _e234: i32 = y_21;
            let _e235: i32 = z_19;
            let _e237: u32 = cellHash(_e233, _e234, _e235, 9i);
            h_1 = _e237;
            let _e239: i32 = j;
            if (_e239 > 0i) {
                let _e242: u32 = h_1;
                let _e244: i32 = j;
                let _e251: i32 = j;
                local_3 = (i32((_e242 % u32(((2i * _e244) + 1i)))) - _e251);
            } else {
                local_3 = 0i;
            }
            let _e255: i32 = local_3;
            ox = _e255;
            let _e257: i32 = j;
            if (_e257 > 0i) {
                let _e260: u32 = h_1;
                let _e265: i32 = j;
                let _e272: i32 = j;
                local_4 = (i32(((_e260 >> 8u) % u32(((2i * _e265) + 1i)))) - _e272);
            } else {
                local_4 = 0i;
            }
            let _e276: i32 = local_4;
            oy = _e276;
            let _e278: i32 = dx_3;
            let _e279: i32 = ox;
            ax = (_e278 - _e279);
            let _e282: i32 = dy_3;
            let _e283: i32 = oy;
            ay = (_e282 - _e283);
            let _e286: i32 = ax;
            let _e289: i32 = ax;
            let _e290: i32 = S;
            let _e293: i32 = ay;
            let _e297: i32 = ay;
            let _e298: i32 = S;
            if ((((_e286 >= 0i) && (_e289 < _e290)) && (_e293 >= 0i)) && (_e297 < _e298)) {
                {
                    let _e301: i32 = accent;
                    let _e304: i32 = S;
                    let _e306: i32 = ax;
                    let _e309: i32 = S;
                    let _e311: i32 = ay;
                    let _e315: vec4<f32> = textureLoad(wgpu_vineTex, vec2<i32>((((_e301 - 1i) * _e304) + _e306), ((9i * _e309) + _e311)), 0i);
                    a_2 = _e315;
                    let _e317: vec4<f32> = a_2;
                    if (_e317.w >= 0.5f) {
                        {
                            let _e321: vec4<f32> = a_2;
                            t = _e321;
                        }
                    }
                }
            }
        }
    }
    let _e322: vec4<f32> = t;
    let _e324: vec3<f32> = srgbToLinear(_e322.xyz);
    let _e325: vec4<f32> = t;
    return vec4<f32>(_e324.x, _e324.y, _e324.z, _e325.w);
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
    let _e181: i32 = dx_5;
    let _e182: i32 = part_1;
    let _e185: i32 = glyph_1;
    let _e187: i32 = S;
    let _e188: i32 = RISE;
    let _e191: i32 = row_2;
    let _e195: vec4<u32> = textureLoad(wgpu_glyphTex, vec2<i32>(_e181, ((((_e182 * 8i) + _e185) * (_e187 + _e188)) + _e191)), 0i);
    return _e195.x;
}

fn cordCell(v_17: vec4<u32>) -> bool {
    var v_18: vec4<u32>;

    v_18 = v_17;
    let _e177: vec4<u32> = v_18;
    let _e178: i32 = partOf(_e177);
    let _e182: vec4<u32> = v_18;
    let _e187: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(0i, i32(_e182.w)), 0i);
    return (((u.tex.z > 0i) && (_e178 == CROWN)) && (i32((_e187.w + 0.5f)) == ROLE_DRAPE));
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
    let _e180: i32 = S;
    let _e181: i32 = w;
    free = max((_e180 - _e181), 0i);
    let _e186: i32 = free;
    span = i32((((f32(_e186) * f32(((u.tex.w >> 8u) & 255i))) / 100f) + 0.5f));
    let _e204: i32 = free;
    let _e205: i32 = span;
    lo = ((_e204 - _e205) / 2i);
    let _e210: i32 = x_21;
    let _e211: i32 = wrapX(_e210);
    let _e212: i32 = yf_1;
    let _e213: i32 = z_21;
    let _e215: u32 = cellHash(_e211, _e212, _e213, 77i);
    h_2 = _e215;
    let _e217: i32 = lo;
    let _e218: u32 = h_2;
    let _e219: i32 = span;
    let _e228: i32 = w;
    return (f32((_e217 + i32((_e218 % u32((_e219 + 1i)))))) + (0.5f * f32(_e228)));
}

fn cordEnds(x_22: i32, y_22: i32, z_22: i32) -> vec2<f32> {
    var x_23: i32;
    var y_23: i32;
    var z_23: i32;

    x_23 = x_22;
    y_23 = y_22;
    z_23 = z_22;
    let _e176: i32 = x_23;
    let _e177: i32 = y_23;
    let _e180: i32 = z_23;
    let _e181: f32 = cordJoin(_e176, (_e177 + 1i), _e180);
    let _e182: i32 = x_23;
    let _e183: i32 = y_23;
    let _e184: i32 = z_23;
    let _e185: f32 = cordJoin(_e182, _e183, _e184);
    return vec2<f32>(_e181, _e185);
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
        let _e183: f32 = t_2;
        let _e184: f32 = t_2;
        let _e188: f32 = t_2;
        local_5 = ((_e183 * _e184) * (3f - (2f * _e188)));
    } else {
        let _e192: f32 = t_2;
        local_5 = _e192;
    }
    let _e194: f32 = local_5;
    e = _e194;
    let _e196: f32 = a_4;
    let _e197: f32 = b_1;
    let _e198: f32 = e;
    return mix(_e196, _e197, _e198);
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
    let _e183: f32 = a_6;
    let _e184: f32 = b_3;
    let _e185: f32 = t_4;
    let _e186: f32 = cordAt(_e183, _e184, _e185);
    c_2 = _e186;
    let _e189: i32 = S;
    dt = (1f / f32(_e189));
    let _e193: f32 = c_2;
    let _e194: f32 = a_6;
    let _e195: f32 = b_3;
    let _e196: f32 = t_4;
    let _e197: f32 = dt;
    let _e201: f32 = cordAt(_e194, _e195, max((_e196 - _e197), 0f));
    up = abs((_e193 - _e201));
    let _e205: f32 = a_6;
    let _e206: f32 = b_3;
    let _e207: f32 = t_4;
    let _e208: f32 = dt;
    let _e212: f32 = cordAt(_e205, _e206, min((_e207 + _e208), 1f));
    let _e213: f32 = c_2;
    down = abs((_e212 - _e213));
    let _e218: f32 = up;
    let _e219: f32 = down;
    ext = max(((0.5f * max(_e218, _e219)) - 0.5f), 0f);
    let _e227: f32 = xp_1;
    let _e228: f32 = c_2;
    r = (_e227 - _e228);
    let _e231: f32 = r;
    let _e234: f32 = w_1;
    let _e236: f32 = ext;
    let _e239: f32 = r;
    let _e241: f32 = w_1;
    let _e243: f32 = ext;
    return ((_e231 >= ((-0.5f * _e234) - _e236)) && (_e239 < ((0.5f * _e241) + _e243)));
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
    let _e184: bool = walk_1;
    if _e184 {
        let _e185: f32 = a_7;
        let _e186: i32 = S;
        let _e193: i32 = S;
        local_6 = min(_e185, max((f32((_e186 - u.tex.z)) / (2f * f32(_e193))), 0f));
    } else {
        let _e200: f32 = a_7;
        local_6 = _e200;
    }
    let _e202: f32 = local_6;
    return _e202;
}

fn cordPhase(x_24: i32, z_24: i32, k: i32) -> f32 {
    var x_25: i32;
    var z_25: i32;
    var k_1: i32;

    x_25 = x_24;
    z_25 = z_24;
    k_1 = k;
    let _e176: i32 = x_25;
    let _e177: i32 = wrapX(_e176);
    let _e178: i32 = k_1;
    let _e179: i32 = z_25;
    let _e181: u32 = cellHash(_e177, _e178, _e179, 78i);
    return (f32((_e181 >> 8u)) / 16777216f);
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
    let _e177: i32 = k_3;
    o = f32((1i << u32(_e177)));
    let _e182: i32 = k_3;
    if ((_e182 == 0i) || ((u.tex.w & 2i) == 0i)) {
        {
            let _e193: f32 = o;
            return _e193;
        }
    }
    let _e194: i32 = x_27;
    let _e195: i32 = wrapX(_e194);
    let _e196: i32 = k_3;
    let _e197: i32 = z_27;
    let _e199: u32 = cellHash(_e195, _e196, _e197, 79i);
    h_3 = (f32((_e199 >> 8u)) / 16777216f);
    let _e206: f32 = o;
    let _e210: f32 = h_3;
    return (_e206 * (1.15f + (0.05f * ((2f * _e210) - 1f))));
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
    let _e178: f32 = cordPeriod();
    p_4 = max(_e178, 0.25f);
    let _e188: i32 = cordOctaves();
    n = _e188;
    loop {
        let _e192: i32 = k_4;
        let _e193: i32 = n;
        if !((_e192 < _e193)) {
            break;
        }
        {
            let _e199: f32 = sum;
            let _e200: f32 = g;
            let _e202: f32 = yw_1;
            let _e203: i32 = x_29;
            let _e204: i32 = z_29;
            let _e205: i32 = k_4;
            let _e206: f32 = cordFreq(_e203, _e204, _e205);
            let _e208: f32 = p_4;
            let _e210: i32 = x_29;
            let _e211: i32 = z_29;
            let _e212: i32 = k_4;
            let _e213: f32 = cordPhase(_e210, _e211, _e212);
            sum = (_e199 + (_e200 * sin((6.2831855f * (((_e202 * _e206) / _e208) + _e213)))));
            let _e219: f32 = gs;
            let _e220: f32 = g;
            gs = (_e219 + _e220);
            let _e222: f32 = g;
            g = (_e222 * 0.5f);
        }
        continuing {
            let _e196: i32 = k_4;
            k_4 = (_e196 + 1i);
        }
    }
    let _e225: i32 = S;
    let _e228: f32 = amp_1;
    let _e229: f32 = sum;
    let _e231: f32 = gs;
    return (f32(_e225) * (0.5f + ((_e228 * _e229) / _e231)));
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
    let _e178: vec3<i32> = p_6;
    let _e180: vec3<i32> = p_6;
    let _e182: vec3<i32> = p_6;
    let _e187: f32 = t_6;
    let _e189: f32 = amp_3;
    let _e190: f32 = cordWaveAt(_e178.x, _e180.z, ((f32(_e182.y) + 1f) - _e187), _e189);
    let _e191: vec2<f32> = sh_1;
    let _e193: vec2<f32> = sh_1;
    let _e195: f32 = t_6;
    return (_e190 + mix(_e191.x, _e193.y, _e195));
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
    let _e184: f32 = up_2;
    let _e185: f32 = down_2;
    ext_1 = max(((0.5f * max(_e184, _e185)) - 0.5f), 0f);
    let _e193: f32 = xp_3;
    let _e194: f32 = c_4;
    r_1 = (_e193 - _e194);
    let _e197: f32 = r_1;
    let _e200: f32 = w_2;
    let _e202: f32 = ext_1;
    let _e205: f32 = r_1;
    let _e207: f32 = w_2;
    let _e209: f32 = ext_1;
    return ((_e197 >= ((-0.5f * _e200) - _e202)) && (_e205 < ((0.5f * _e207) + _e209)));
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
    let _e181: i32 = S;
    dt_1 = (1f / f32(_e181));
    let _e185: vec3<i32> = p_8;
    let _e186: f32 = t_8;
    let _e187: f32 = amp_5;
    let _e188: vec2<f32> = sh_3;
    let _e189: f32 = cordWaveCentre(_e185, _e186, _e187, _e188);
    c_5 = _e189;
    let _e191: f32 = c_5;
    let _e192: vec3<i32> = p_8;
    let _e193: f32 = t_8;
    let _e194: f32 = dt_1;
    let _e196: f32 = amp_5;
    let _e197: vec2<f32> = sh_3;
    let _e198: f32 = cordWaveCentre(_e192, (_e193 - _e194), _e196, _e197);
    up_3 = abs((_e191 - _e198));
    let _e202: vec3<i32> = p_8;
    let _e203: f32 = t_8;
    let _e204: f32 = dt_1;
    let _e206: f32 = amp_5;
    let _e207: vec2<f32> = sh_3;
    let _e208: f32 = cordWaveCentre(_e202, (_e203 + _e204), _e206, _e207);
    let _e209: f32 = c_5;
    down_3 = abs((_e208 - _e209));
    let _e213: f32 = c_5;
    let _e214: f32 = up_3;
    let _e215: f32 = down_3;
    let _e216: f32 = xp_5;
    let _e217: bool = cordSpan(_e213, _e214, _e215, _e216);
    return _e217;
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
    let _e180: bool = cordWave();
    if _e180 {
        {
            let _e181: i32 = dy_5;
            let _e185: i32 = S;
            t_9 = ((f32(_e181) + 0.5f) / f32(_e185));
            let _e189: i32 = x_31;
            let _e190: i32 = y_25;
            let _e191: i32 = z_31;
            let _e193: f32 = t_9;
            let _e194: i32 = dx_7;
            let _e199: f32 = cordAmp(true);
            let _e202: bool = cordWaveCovers(vec3<i32>(_e189, _e190, _e191), _e193, (f32(_e194) + 0.5f), _e199, vec2(0f));
            return _e202;
        }
    }
    let _e203: i32 = x_31;
    let _e204: i32 = y_25;
    let _e205: i32 = z_31;
    let _e206: vec2<f32> = cordEnds(_e203, _e204, _e205);
    e_1 = _e206;
    let _e208: vec2<f32> = e_1;
    let _e210: vec2<f32> = e_1;
    let _e212: i32 = dy_5;
    let _e216: i32 = S;
    let _e219: i32 = dx_7;
    let _e223: bool = cordCovers(_e208.x, _e210.y, ((f32(_e212) + 0.5f) / f32(_e216)), (f32(_e219) + 0.5f));
    return _e223;
}

fn cordGlyphTexel(v_19: vec4<u32>, dx_8: i32, row_3: i32) -> u32 {
    var v_20: vec4<u32>;
    var dx_9: i32;
    var row_4: i32;
    var local_7: u32;

    v_20 = v_19;
    dx_9 = dx_8;
    row_4 = row_3;
    let _e176: vec4<u32> = v_20;
    let _e177: bool = cordCell(_e176);
    if _e177 {
        let _e178: vec4<u32> = v_20;
        let _e179: i32 = partOf(_e178);
        let _e180: vec4<u32> = v_20;
        let _e181: i32 = glyphOf(_e180);
        let _e182: i32 = S;
        let _e185: i32 = S;
        let _e188: u32 = glyphTexel(_e179, _e181, (_e182 / 2i), (_e185 / 2i));
        local_7 = _e188;
    } else {
        let _e189: vec4<u32> = v_20;
        let _e190: i32 = partOf(_e189);
        let _e191: vec4<u32> = v_20;
        let _e192: i32 = glyphOf(_e191);
        let _e193: i32 = dx_9;
        let _e194: i32 = row_4;
        let _e195: u32 = glyphTexel(_e190, _e192, _e193, _e194);
        local_7 = _e195;
    }
    let _e197: u32 = local_7;
    return _e197;
}

fn strataOf(m_2: i32) -> vec3<f32> {
    var m_3: i32;

    m_3 = m_2;
    let _e172: i32 = m_3;
    if (_e172 == 0i) {
        {
            return u.skyC.xyz;
        }
    }
    let _e178: i32 = m_3;
    if (_e178 == 1i) {
        {
            return u.bedrockC.xyz;
        }
    }
    let _e184: i32 = m_3;
    if (_e184 == 2i) {
        {
            return u.rockC.xyz;
        }
    }
    return u.soilC.xyz;
}

fn holdsPore(m_4: i32) -> bool {
    var m_5: i32;

    m_5 = m_4;
    let _e172: i32 = m_5;
    let _e175: i32 = m_5;
    return ((_e172 == 2i) || (_e175 == 3i));
}

fn hazeAt(zf: f32) -> f32 {
    var zf_1: f32;

    zf_1 = zf;
    let _e172: i32 = D;
    if (_e172 <= 1i) {
        {
            return 0f;
        }
    }
    let _e182: f32 = zf_1;
    let _e183: i32 = D;
    return (clamp(u.knobs.x, 0f, 1f) * clamp((_e182 / f32((_e183 - 1i))), 0f, 1f));
}

fn hazed(c_6: vec3<f32>, t_10: f32) -> vec3<f32> {
    var c_7: vec3<f32>;
    var t_11: f32;

    c_7 = c_6;
    t_11 = t_10;
    let _e174: vec3<f32> = c_7;
    let _e178: f32 = t_11;
    return mix(_e174, u.hazeC.xyz, vec3(clamp(_e178, 0f, 1f)));
}

fn roofShade(gap: i32) -> f32 {
    var gap_1: i32;
    var t_12: f32;

    gap_1 = gap;
    let _e172: i32 = gap_1;
    if (_e172 == 0i) {
        {
            return 1f;
        }
    }
    let _e177: i32 = gap_1;
    t_12 = (1f - exp((-(f32((_e177 - 1i))) / u.roofK.y)));
    let _e197: f32 = t_12;
    return (u.roofK.x + ((1f - u.roofK.x) * _e197));
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
            let _e181: i32 = x_33;
            let _e182: i32 = wrapX(_e181);
            let _e183: i32 = y_27;
            let _e184: i32 = z_33;
            let _e187: vec4<u32> = textureLoad(wgpu_roofTex, vec3<i32>(_e182, _e183, _e184), 0i);
            return i32(_e187.x);
        }
    }
    let _e190: i32 = y_27;
    yy = (_e190 + 1i);
    loop {
        let _e194: i32 = yy;
        let _e195: i32 = H;
        if !((_e194 < _e195)) {
            break;
        }
        {
            let _e201: i32 = x_33;
            let _e202: i32 = yy;
            let _e203: i32 = z_33;
            let _e204: vec4<u32> = at(_e201, _e202, _e203);
            let _e205: bool = solidV(_e204);
            if _e205 {
                {
                    let _e206: i32 = yy;
                    let _e207: i32 = y_27;
                    return (_e206 - _e207);
                }
            }
        }
        continuing {
            let _e198: i32 = yy;
            yy = (_e198 + 1i);
        }
    }
    return 0i;
}

fn styleAt(style: i32, part_2: i32) -> vec3<f32> {
    var style_1: i32;
    var part_3: i32;

    style_1 = style;
    part_3 = part_2;
    let _e174: i32 = part_3;
    let _e175: i32 = style_1;
    let _e178: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(_e174, _e175), 0i);
    return _e178.xyz;
}

fn styleEmit(style_2: i32) -> vec3<f32> {
    var style_3: i32;

    style_3 = style_2;
    let _e173: i32 = style_3;
    let _e176: vec4<f32> = textureLoad(wgpu_styleTex, vec2<i32>(3i, _e173), 0i);
    let _e178: i32 = style_3;
    let _e181: i32 = style_3;
    let _e188: f32 = u.emitK[(_e181 >> 2u)][(_e178 & 3i)];
    return (_e176.xyz * _e188);
}

fn emitsWhole(pigmentAlpha: f32) -> bool {
    var pigmentAlpha_1: f32;

    pigmentAlpha_1 = pigmentAlpha;
    let _e172: f32 = pigmentAlpha_1;
    return (fract(_e172) > 0.125f);
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
    let _e186: u32 = cap;
    if (_e186 == 0u) {
        {
            return -1i;
        }
    }
    let _e191: i32 = z_35;
    let _e192: i32 = H;
    let _e194: i32 = y_29;
    let _e196: i32 = W;
    let _e198: i32 = x_35;
    let _e199: i32 = wrapX(_e198);
    key = (u32(((((_e191 * _e192) + _e194) * _e196) + _e199)) + 1u);
    let _e205: u32 = key;
    let _e209: u32 = cap;
    let _e217: u32 = cap;
    h_4 = (((_e205 * 2654435761u) >> u32((32i - min(i32(firstTrailingBit(_e209)), 31i)))) & (_e217 - 1u));
    loop {
        let _e224: i32 = i;
        if !((_e224 < 32i)) {
            break;
        }
        {
            let _e234: u32 = h_4;
            let _e238: vec4<u32> = global.emitCells[(u.view.z + i32(_e234))];
            c_8 = _e238;
            let _e240: vec4<u32> = c_8;
            let _e242: u32 = key;
            if (_e240.x == _e242) {
                {
                    let _e247: u32 = h_4;
                    return (u.view.z + i32(_e247));
                }
            }
            let _e250: vec4<u32> = c_8;
            if (_e250.x == 0u) {
                {
                    return -1i;
                }
            }
            let _e256: u32 = h_4;
            let _e259: u32 = cap;
            h_4 = ((_e256 + 1u) & (_e259 - 1u));
        }
        continuing {
            let _e228: i32 = i;
            i = (_e228 + 1i);
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
    let _e176: i32 = i_2;
    if (!(LIT) || (_e176 < 0i)) {
        {
            return false;
        }
    }
    let _e181: i32 = i_2;
    let _e183: vec4<u32> = global.emitCells[_e181];
    c_9 = _e183;
    let _e185: vec4<u32> = c_9;
    if ((_e185.w >> 31u) != 0u) {
        {
            return false;
        }
    }
    let _e193: vec4<u32> = c_9;
    rg = unpack2x16float(_e193.y);
    let _e197: vec4<u32> = c_9;
    bo = unpack2x16float(_e197.z);
    let _e201: vec2<f32> = rg;
    let _e202: vec2<f32> = bo;
    (*e_2) = vec3<f32>(_e201.x, _e201.y, _e202.x);
    let _e207: vec2<f32> = bo;
    (*on) = clamp(_e207.y, 0f, 1f);
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
    let _e181: i32 = x_37;
    let _e182: i32 = y_31;
    let _e183: i32 = z_37;
    let _e184: i32 = cellEntry(_e181, _e182, _e183);
    let _e189: bool = entryEmit(_e184, e_3, on_1);
    return _e189;
}

fn entryVineFrom(i_3: i32, v_21: ptr<function, vec4<u32>>) -> f32 {
    var i_4: i32;
    var w_3: u32;
    var s: u32;

    i_4 = i_3;
    let _e174: i32 = i_4;
    if ((_e174 < 0i) || ((u32(u.view.w) & EMIT_VINES) == 0u)) {
        {
            return 0f;
        }
    }
    let _e186: i32 = i_4;
    let _e188: vec4<u32> = global.emitCells[_e186];
    w_3 = _e188.w;
    let _e191: u32 = w_3;
    s = ((_e191 >> 16u) & 255u);
    let _e198: u32 = s;
    if (_e198 == 0u) {
        {
            return 0f;
        }
    }
    let _e203: u32 = w_3;
    (*v_21).z = (_e203 & 255u);
    let _e207: u32 = w_3;
    (*v_21).w = ((_e207 >> 8u) & 255u);
    let _e213: u32 = s;
    return (f32(_e213) / 255f);
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
    let _e188: i32 = x_39;
    let _e189: i32 = y_33;
    let _e190: i32 = z_39;
    let _e191: i32 = cellEntry(_e188, _e189, _e190);
    i_5 = _e191;
    let _e193: i32 = i_5;
    if (_e193 < 0i) {
        {
            return 1f;
        }
    }
    let _e197: i32 = i_5;
    let _e199: vec4<u32> = global.emitCells[_e197];
    p_9 = ((_e199.w >> 24u) & 127u);
    let _e207: u32 = p_9;
    if (_e207 == 0u) {
        local_8 = 1f;
    } else {
        let _e211: u32 = p_9;
        local_8 = (f32((_e211 - 1u)) / 125f);
    }
    let _e218: f32 = local_8;
    return _e218;
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
    let _e189: i32 = dx_11;
    let _e192: i32 = dx_11;
    let _e193: i32 = S;
    if ((!(PLANT_LIGHT) || ((u32(u.view.w) & EMIT_PLANTS) == 0u)) || ((_e189 != 0i) && (_e192 != (_e193 - 1i)))) {
        {
            return 1f;
        }
    }
    let _e200: i32 = x_41;
    let _e201: i32 = y_35;
    let _e202: i32 = z_41;
    let _e203: i32 = cellEntry(_e200, _e201, _e202);
    i_6 = _e203;
    let _e205: i32 = i_6;
    if (_e205 < 0i) {
        {
            return 1f;
        }
    }
    let _e209: i32 = i_6;
    let _e211: vec4<u32> = global.emitCells[_e209];
    w_4 = _e211.w;
    let _e214: u32 = w_4;
    if (((_e214 >> 16u) & 255u) != 0u) {
        {
            return 1f;
        }
    }
    let _e223: i32 = dx_11;
    if (_e223 == 0i) {
        let _e226: u32 = w_4;
        local_9 = (_e226 & 255u);
    } else {
        let _e229: u32 = w_4;
        local_9 = ((_e229 >> 8u) & 255u);
    }
    let _e236: u32 = local_9;
    e_4 = _e236;
    let _e238: u32 = e_4;
    if (_e238 == 0u) {
        local_10 = 1f;
    } else {
        let _e242: u32 = e_4;
        local_10 = (f32((_e242 - 1u)) / 254f);
    }
    let _e249: f32 = local_10;
    return _e249;
}

fn emitterTrans(trans: vec3<f32>, drawn: vec3<f32>, haze: f32) -> vec3<f32> {
    var trans_1: vec3<f32>;
    var drawn_1: vec3<f32>;
    var haze_1: f32;

    trans_1 = trans;
    drawn_1 = drawn;
    haze_1 = haze;
    let _e178: vec3<f32> = trans_1;
    let _e179: vec3<f32> = drawn_1;
    let _e180: f32 = haze_1;
    return (-((_e178 * max((_e179 - (clamp(_e180, 0f, 1f) * u.hazeC.xyz)), vec3(0f)))) - vec3(1f));
}

fn vineEmits(t_13: vec4<f32>) -> bool {
    var t_14: vec4<f32>;

    t_14 = t_13;
    let _e175: vec4<f32> = t_14;
    let _e179: vec4<f32> = t_14;
    return ((_e175.w >= 0.5f) && (_e179.w < 0.998f));
}

fn plantLit(c_10: vec3<f32>, shade: f32) -> vec3<f32> {
    var c_11: vec3<f32>;
    var shade_1: f32;
    var lit: vec3<f32>;
    var local_11: vec3<f32>;

    c_11 = c_10;
    shade_1 = shade;
    let _e177: vec3<f32> = c_11;
    lit = mix((_e177 * u.plantA.x), u.lightC.xyz, vec3(u.plantA.y));
    let _e191: f32 = shade_1;
    if (_e191 < 1f) {
        let _e194: vec3<f32> = c_11;
        let _e195: vec3<f32> = lit;
        let _e196: f32 = shade_1;
        local_11 = mix(_e194, _e195, vec3(_e196));
    } else {
        let _e199: vec3<f32> = lit;
        local_11 = _e199;
    }
    let _e201: vec3<f32> = local_11;
    return _e201;
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
    let _e189: i32 = slot_5;
    let _e190: bool = texOn(_e189);
    if !(_e190) {
        {
            let _e192: i32 = m_7;
            let _e193: vec3<f32> = strataOf(_e192);
            return _e193;
        }
    }
    let _e194: i32 = slot_5;
    let _e195: i32 = x_43;
    let _e196: i32 = y_37;
    let _e197: i32 = z_43;
    let _e198: i32 = salt_7;
    let _e199: i32 = dx_13;
    let _e200: i32 = dy_7;
    let _e201: vec4<f32> = faceTexel(_e194, _e195, _e196, _e197, _e198, _e199, _e200);
    let _e203: vec3<f32> = srgbToLinear(_e201.xyz);
    return _e203;
}

fn blockBody(v_22: vec4<u32>, base: vec3<f32>) -> vec3<f32> {
    var v_23: vec4<u32>;
    var base_1: vec3<f32>;
    var m_8: i32;
    var local_12: f32;
    var wet: f32;

    v_23 = v_22;
    base_1 = base;
    let _e177: vec4<u32> = v_23;
    let _e178: i32 = matOf(_e177);
    m_8 = _e178;
    let _e180: i32 = m_8;
    let _e181: bool = holdsPore(_e180);
    if _e181 {
        let _e182: vec4<u32> = v_23;
        local_12 = clamp((f32(_e182.z) / 255f), 0f, 1f);
    } else {
        local_12 = 0f;
    }
    let _e192: f32 = local_12;
    wet = _e192;
    let _e194: vec3<f32> = base_1;
    let _e198: f32 = wet;
    return mix(_e194, u.waterDeepC.xyz, vec3((_e198 * u.shadeB.w)));
}

fn blockLit(body: vec3<f32>, shade_2: f32) -> vec3<f32> {
    var body_1: vec3<f32>;
    var shade_3: f32;
    var lit_1: vec3<f32>;
    var local_13: vec3<f32>;

    body_1 = body;
    shade_3 = shade_2;
    let _e177: vec3<f32> = body_1;
    lit_1 = mix((_e177 * u.shadeA.x), u.lightC.xyz, vec3(u.shadeA.y));
    let _e191: f32 = shade_3;
    if (_e191 < 1f) {
        let _e194: vec3<f32> = body_1;
        let _e195: vec3<f32> = lit_1;
        let _e196: f32 = shade_3;
        local_13 = mix(_e194, _e195, vec3(_e196));
    } else {
        let _e199: vec3<f32> = lit_1;
        local_13 = _e199;
    }
    let _e201: vec3<f32> = local_13;
    return _e201;
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
    let _e179: i32 = y_39;
    let _e180: bool = inY(_e179);
    let _e182: i32 = z_45;
    let _e186: i32 = z_45;
    let _e187: i32 = D;
    if ((!(_e180) || (_e182 < 0i)) || (_e186 >= _e187)) {
        {
            return 0f;
        }
    }
    let _e191: i32 = x_45;
    let _e192: i32 = y_39;
    let _e193: i32 = z_45;
    let _e194: vec4<u32> = at(_e191, _e192, _e193);
    v_24 = _e194;
    if !(PLANT_LIGHT) {
        {
            let _e197: vec4<u32> = v_24;
            let _e198: bool = solidV(_e197);
            let _e199: vec4<u32> = v_24;
            let _e200: i32 = partOf(_e199);
            let _e201: bool = isBlockPart(_e200);
            let _e202: vec4<u32> = v_24;
            let _e203: bool = softMark(_e202);
            let _e206: vec4<u32> = v_24;
            let _e207: bool = cordCell(_e206);
            if (_e198 || ((_e201 && !(_e203)) && !(_e207))) {
                local_14 = 1f;
            } else {
                local_14 = 0f;
            }
            let _e214: f32 = local_14;
            return _e214;
        }
    }
    let _e215: vec4<u32> = v_24;
    let _e216: bool = solidV(_e215);
    if _e216 {
        {
            return 1f;
        }
    }
    let _e218: vec4<u32> = v_24;
    let _e219: i32 = partOf(_e218);
    p_10 = _e219;
    let _e221: i32 = p_10;
    let _e222: bool = isBlockPart(_e221);
    let _e224: vec4<u32> = v_24;
    let _e225: bool = softMark(_e224);
    let _e227: vec4<u32> = v_24;
    let _e228: bool = cordCell(_e227);
    if ((!(_e222) || _e225) || _e228) {
        {
            return 0f;
        }
    }
    let _e231: i32 = p_10;
    let _e232: bool = plantBlockPart(_e231);
    if _e232 {
        let _e233: i32 = x_45;
        let _e234: i32 = y_39;
        let _e235: i32 = z_45;
        let _e236: f32 = plantSize(_e233, _e234, _e235);
        local_15 = _e236;
    } else {
        local_15 = 1f;
    }
    let _e239: f32 = local_15;
    return _e239;
}

fn occluderAt(c_12: vec3<i32>) -> f32 {
    var c_13: vec3<i32>;

    c_13 = c_12;
    let _e175: vec3<i32> = c_13;
    let _e177: vec3<i32> = c_13;
    let _e179: vec3<i32> = c_13;
    let _e181: f32 = occluder(_e175.x, _e177.y, _e179.z);
    return _e181;
}

fn creaseWidth(rows: i32) -> i32 {
    var rows_1: i32;

    rows_1 = rows;
    let _e176: i32 = S;
    let _e183: i32 = rows_1;
    return min(max(1i, ((_e176 + 4i) / 8i)), max(1i, (_e183 / 2i)));
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
    let _e199: i32 = du_1;
    let _e200: i32 = b_5;
    inL = (_e199 < _e200);
    let _e203: i32 = du_1;
    let _e204: i32 = nu_1;
    let _e205: i32 = b_5;
    inR = (_e203 >= (_e204 - _e205));
    let _e209: i32 = dv_1;
    let _e210: i32 = b_5;
    inH = (_e209 < _e210);
    let _e213: i32 = dv_1;
    let _e214: i32 = nv_1;
    let _e215: i32 = b_5;
    inLo = (_e213 >= (_e214 - _e215));
    if !(PLANT_LIGHT) {
        {
            let _e220: bool = inL;
            let _e221: f32 = l_1;
            let _e225: bool = inR;
            let _e226: f32 = r_3;
            let _e231: bool = inH;
            let _e232: f32 = hi_1;
            let _e237: bool = inLo;
            let _e238: f32 = lo_2;
            let _e243: bool = inL;
            let _e244: bool = inLo;
            let _e246: f32 = cLL_1;
            let _e251: bool = inR;
            let _e252: bool = inLo;
            let _e254: f32 = cRL_1;
            let _e259: bool = inL;
            let _e260: bool = inH;
            let _e262: f32 = cLH_1;
            let _e267: bool = inR;
            let _e268: bool = inH;
            let _e270: f32 = cRH_1;
            band = ((((((((_e220 && (_e221 > 0.5f)) || (_e225 && (_e226 > 0.5f))) || (_e231 && (_e232 > 0.5f))) || (_e237 && (_e238 > 0.5f))) || ((_e243 && _e244) && (_e246 > 0.5f))) || ((_e251 && _e252) && (_e254 > 0.5f))) || ((_e259 && _e260) && (_e262 > 0.5f))) || ((_e267 && _e268) && (_e270 > 0.5f)));
            let _e276: bool = band;
            if _e276 {
                local_16 = (1f - u.lightK.w);
            } else {
                local_16 = 1f;
            }
            let _e284: f32 = local_16;
            return _e284;
        }
    }
    let _e285: bool = inL;
    if _e285 {
        let _e286: f32 = l_1;
        local_17 = _e286;
    } else {
        local_17 = 0f;
    }
    let _e289: f32 = local_17;
    let _e290: bool = inR;
    if _e290 {
        let _e291: f32 = r_3;
        local_18 = _e291;
    } else {
        local_18 = 0f;
    }
    let _e294: f32 = local_18;
    let _e296: bool = inH;
    if _e296 {
        let _e297: f32 = hi_1;
        local_19 = _e297;
    } else {
        local_19 = 0f;
    }
    let _e300: f32 = local_19;
    let _e301: bool = inLo;
    if _e301 {
        let _e302: f32 = lo_2;
        local_20 = _e302;
    } else {
        local_20 = 0f;
    }
    let _e305: f32 = local_20;
    k_5 = max(max(_e289, _e294), max(_e300, _e305));
    let _e309: f32 = k_5;
    let _e310: bool = inL;
    let _e311: bool = inLo;
    if (_e310 && _e311) {
        let _e313: f32 = cLL_1;
        local_21 = _e313;
    } else {
        local_21 = 0f;
    }
    let _e316: f32 = local_21;
    let _e317: bool = inR;
    let _e318: bool = inLo;
    if (_e317 && _e318) {
        let _e320: f32 = cRL_1;
        local_22 = _e320;
    } else {
        local_22 = 0f;
    }
    let _e323: f32 = local_22;
    let _e325: bool = inL;
    let _e326: bool = inH;
    if (_e325 && _e326) {
        let _e328: f32 = cLH_1;
        local_23 = _e328;
    } else {
        local_23 = 0f;
    }
    let _e331: f32 = local_23;
    let _e332: bool = inR;
    let _e333: bool = inH;
    if (_e332 && _e333) {
        let _e335: f32 = cRH_1;
        local_24 = _e335;
    } else {
        local_24 = 0f;
    }
    let _e338: f32 = local_24;
    k_5 = max(_e309, max(max(_e316, _e323), max(_e331, _e338)));
    let _e346: f32 = k_5;
    return (1f - (u.lightK.w * _e346));
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
    let _e197: f32 = l_3;
    let _e198: f32 = r_5;
    let _e199: f32 = lo_4;
    let _e200: f32 = hi_3;
    let _e201: f32 = cLL_3;
    let _e202: f32 = cRL_3;
    let _e203: f32 = cLH_3;
    let _e204: f32 = cRH_3;
    let _e205: i32 = du_3;
    let _e206: i32 = dv_3;
    let _e207: i32 = nu_3;
    let _e208: i32 = nv_3;
    let _e209: i32 = nv_3;
    let _e210: i32 = creaseWidth(_e209);
    let _e211: f32 = creaseBands(_e197, _e198, _e199, _e200, _e201, _e202, _e203, _e204, _e205, _e206, _e207, _e208, _e210);
    return _e211;
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
    let _e187: i32 = nv_5;
    let _e188: i32 = creaseWidth(_e187);
    b_6 = _e188;
    let _e190: i32 = du_5;
    let _e191: i32 = b_6;
    inL_1 = (_e190 < _e191);
    let _e194: i32 = du_5;
    let _e195: i32 = nu_5;
    let _e196: i32 = b_6;
    inR_1 = (_e194 >= (_e195 - _e196));
    let _e200: i32 = dv_5;
    let _e201: i32 = b_6;
    inH_1 = (_e200 < _e201);
    let _e204: i32 = dv_5;
    let _e205: i32 = nv_5;
    let _e206: i32 = b_6;
    inLo_1 = (_e204 >= (_e205 - _e206));
    let _e212: bool = inL_1;
    if _e212 {
        {
            let _e213: f32 = k_6;
            let _e214: vec3<i32> = o_2;
            let _e215: vec3<i32> = U_1;
            let _e217: f32 = occluderAt((_e214 - _e215));
            k_6 = max(_e213, _e217);
        }
    }
    let _e219: bool = inR_1;
    if _e219 {
        {
            let _e220: f32 = k_6;
            let _e221: vec3<i32> = o_2;
            let _e222: vec3<i32> = U_1;
            let _e224: f32 = occluderAt((_e221 + _e222));
            k_6 = max(_e220, _e224);
        }
    }
    let _e226: bool = inH_1;
    if _e226 {
        {
            let _e227: f32 = k_6;
            let _e228: vec3<i32> = o_2;
            let _e229: vec3<i32> = V_1;
            let _e231: f32 = occluderAt((_e228 + _e229));
            k_6 = max(_e227, _e231);
        }
    }
    let _e233: bool = inLo_1;
    if _e233 {
        {
            let _e234: f32 = k_6;
            let _e235: vec3<i32> = o_2;
            let _e236: vec3<i32> = V_1;
            let _e238: f32 = occluderAt((_e235 - _e236));
            k_6 = max(_e234, _e238);
        }
    }
    let _e240: bool = inL_1;
    let _e241: bool = inLo_1;
    if (_e240 && _e241) {
        {
            let _e243: f32 = k_6;
            let _e244: vec3<i32> = o_2;
            let _e245: vec3<i32> = U_1;
            let _e247: vec3<i32> = V_1;
            let _e249: f32 = occluderAt(((_e244 - _e245) - _e247));
            k_6 = max(_e243, _e249);
        }
    }
    let _e251: bool = inR_1;
    let _e252: bool = inLo_1;
    if (_e251 && _e252) {
        {
            let _e254: f32 = k_6;
            let _e255: vec3<i32> = o_2;
            let _e256: vec3<i32> = U_1;
            let _e258: vec3<i32> = V_1;
            let _e260: f32 = occluderAt(((_e255 + _e256) - _e258));
            k_6 = max(_e254, _e260);
        }
    }
    let _e262: bool = inL_1;
    let _e263: bool = inH_1;
    if (_e262 && _e263) {
        {
            let _e265: f32 = k_6;
            let _e266: vec3<i32> = o_2;
            let _e267: vec3<i32> = U_1;
            let _e269: vec3<i32> = V_1;
            let _e271: f32 = occluderAt(((_e266 - _e267) + _e269));
            k_6 = max(_e265, _e271);
        }
    }
    let _e273: bool = inR_1;
    let _e274: bool = inH_1;
    if (_e273 && _e274) {
        {
            let _e276: f32 = k_6;
            let _e277: vec3<i32> = o_2;
            let _e278: vec3<i32> = U_1;
            let _e280: vec3<i32> = V_1;
            let _e282: f32 = occluderAt(((_e277 + _e278) + _e280));
            k_6 = max(_e276, _e282);
        }
    }
    let _e288: f32 = k_6;
    return (1f - (u.lightK.w * _e288));
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
    let _e183: u32 = mask_1;
    let _e187: u32 = mask_1;
    let _e194: u32 = mask_1;
    let _e201: u32 = mask_1;
    let _e208: u32 = mask_1;
    let _e215: u32 = mask_1;
    let _e222: u32 = mask_1;
    let _e229: u32 = mask_1;
    let _e236: i32 = dx_15;
    let _e237: i32 = dy_9;
    let _e238: vec2<i32> = size_1;
    let _e240: vec2<i32> = size_1;
    let _e242: vec2<i32> = size_1;
    let _e244: vec2<i32> = size_1;
    let _e247: i32 = creaseWidth(min(_e242.x, _e244.y));
    let _e248: f32 = creaseBands(f32((_e183 & 1u)), f32(((_e187 >> 1u) & 1u)), f32(((_e194 >> 2u) & 1u)), f32(((_e201 >> 3u) & 1u)), f32(((_e208 >> 4u) & 1u)), f32(((_e215 >> 5u) & 1u)), f32(((_e222 >> 6u) & 1u)), f32(((_e229 >> 7u) & 1u)), _e236, _e237, _e238.x, _e240.y, _e247);
    return _e248;
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
            let _e186: i32 = z_47;
            p_11 = (_e186 - 1i);
            let _e190: i32 = x_47;
            let _e193: i32 = y_41;
            let _e194: i32 = p_11;
            let _e195: f32 = occluder((_e190 - 1i), _e193, _e194);
            let _e196: i32 = x_47;
            let _e199: i32 = y_41;
            let _e200: i32 = p_11;
            let _e201: f32 = occluder((_e196 + 1i), _e199, _e200);
            let _e202: i32 = x_47;
            let _e203: i32 = y_41;
            let _e206: i32 = p_11;
            let _e207: f32 = occluder(_e202, (_e203 - 1i), _e206);
            let _e208: i32 = x_47;
            let _e209: i32 = y_41;
            let _e212: i32 = p_11;
            let _e213: f32 = occluder(_e208, (_e209 + 1i), _e212);
            let _e214: i32 = x_47;
            let _e217: i32 = y_41;
            let _e220: i32 = p_11;
            let _e221: f32 = occluder((_e214 - 1i), (_e217 - 1i), _e220);
            let _e222: i32 = x_47;
            let _e225: i32 = y_41;
            let _e228: i32 = p_11;
            let _e229: f32 = occluder((_e222 + 1i), (_e225 - 1i), _e228);
            let _e230: i32 = x_47;
            let _e233: i32 = y_41;
            let _e236: i32 = p_11;
            let _e237: f32 = occluder((_e230 - 1i), (_e233 + 1i), _e236);
            let _e238: i32 = x_47;
            let _e241: i32 = y_41;
            let _e244: i32 = p_11;
            let _e245: f32 = occluder((_e238 + 1i), (_e241 + 1i), _e244);
            let _e246: i32 = dx_17;
            let _e247: i32 = dy_11;
            let _e248: i32 = S;
            let _e249: i32 = S;
            let _e250: f32 = crease(_e195, _e201, _e207, _e213, _e221, _e229, _e237, _e245, _e246, _e247, _e248, _e249);
            return _e250;
        }
    }
    let _e251: i32 = x_47;
    let _e252: i32 = y_41;
    let _e253: i32 = z_47;
    let _e265: i32 = dx_17;
    let _e266: i32 = dy_11;
    let _e267: i32 = S;
    let _e268: i32 = S;
    let _e269: f32 = creaseOpen(vec3<i32>(_e251, _e252, (_e253 - 1i)), vec3<i32>(1i, 0i, 0i), vec3<i32>(0i, 1i, 0i), _e265, _e266, _e267, _e268);
    return _e269;
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
            let _e186: i32 = y_43;
            p_12 = (_e186 + 1i);
            let _e190: i32 = x_49;
            let _e193: i32 = p_12;
            let _e194: i32 = z_49;
            let _e195: f32 = occluder((_e190 - 1i), _e193, _e194);
            let _e196: i32 = x_49;
            let _e199: i32 = p_12;
            let _e200: i32 = z_49;
            let _e201: f32 = occluder((_e196 + 1i), _e199, _e200);
            let _e202: i32 = x_49;
            let _e203: i32 = p_12;
            let _e204: i32 = z_49;
            let _e207: f32 = occluder(_e202, _e203, (_e204 - 1i));
            let _e208: i32 = x_49;
            let _e209: i32 = p_12;
            let _e210: i32 = z_49;
            let _e213: f32 = occluder(_e208, _e209, (_e210 + 1i));
            let _e214: i32 = x_49;
            let _e217: i32 = p_12;
            let _e218: i32 = z_49;
            let _e221: f32 = occluder((_e214 - 1i), _e217, (_e218 - 1i));
            let _e222: i32 = x_49;
            let _e225: i32 = p_12;
            let _e226: i32 = z_49;
            let _e229: f32 = occluder((_e222 + 1i), _e225, (_e226 - 1i));
            let _e230: i32 = x_49;
            let _e233: i32 = p_12;
            let _e234: i32 = z_49;
            let _e237: f32 = occluder((_e230 - 1i), _e233, (_e234 + 1i));
            let _e238: i32 = x_49;
            let _e241: i32 = p_12;
            let _e242: i32 = z_49;
            let _e245: f32 = occluder((_e238 + 1i), _e241, (_e242 + 1i));
            let _e246: i32 = dx_19;
            let _e247: i32 = dy_13;
            let _e248: i32 = S;
            let _e249: i32 = RISE;
            let _e250: f32 = crease(_e195, _e201, _e207, _e213, _e221, _e229, _e237, _e245, _e246, _e247, _e248, _e249);
            return _e250;
        }
    }
    let _e251: i32 = x_49;
    let _e252: i32 = y_43;
    let _e255: i32 = z_49;
    let _e265: i32 = dx_19;
    let _e266: i32 = dy_13;
    let _e267: i32 = S;
    let _e268: i32 = RISE;
    let _e269: f32 = creaseOpen(vec3<i32>(_e251, (_e252 + 1i), _e255), vec3<i32>(1i, 0i, 0i), vec3<i32>(0i, 0i, 1i), _e265, _e266, _e267, _e268);
    return _e269;
}

fn skyOpen(x_50: i32, y_44: i32, z_50: i32) -> f32 {
    var x_51: i32;
    var y_45: i32;
    var z_51: i32;

    x_51 = x_50;
    y_45 = y_44;
    z_51 = z_50;
    let _e179: i32 = y_45;
    let _e180: i32 = H;
    let _e182: i32 = z_51;
    if ((_e179 >= _e180) || (_e182 < 0i)) {
        {
            return 1f;
        }
    }
    let _e187: i32 = y_45;
    let _e190: i32 = z_51;
    let _e191: i32 = D;
    if ((_e187 < 0i) || (_e190 >= _e191)) {
        {
            return 0f;
        }
    }
    let _e195: i32 = x_51;
    let _e196: i32 = wrapX(_e195);
    let _e197: i32 = y_45;
    let _e198: i32 = z_51;
    let _e201: vec4<u32> = textureLoad(wgpu_skyTex, vec3<i32>(_e196, _e197, _e198), 0i);
    return mix((f32(_e201.x) / 255f), 1f, u.clock.w);
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
    let _e179: i32 = z_53;
    let _e182: i32 = z_53;
    let _e183: i32 = D;
    if ((_e179 < 0i) || (_e182 >= _e183)) {
        {
            return 1f;
        }
    }
    let _e187: i32 = x_53;
    let _e188: i32 = wrapX(_e187);
    let _e190: i32 = z_53;
    let _e194: vec4<u32> = textureLoad(wgpu_canopyTex, vec2<i32>(_e188, (2i * _e190)), 0i);
    a_8 = _e194;
    let _e196: vec4<u32> = a_8;
    let _e199: i32 = yv_1;
    if (i32(_e196.x) <= _e199) {
        {
            return 1f;
        }
    }
    let _e202: vec4<u32> = a_8;
    let _e205: i32 = yv_1;
    if (i32(_e202.z) <= _e205) {
        {
            let _e207: vec4<u32> = a_8;
            return (f32(_e207.y) / 255f);
        }
    }
    let _e212: i32 = x_53;
    let _e213: i32 = wrapX(_e212);
    let _e215: i32 = z_53;
    let _e221: vec4<u32> = textureLoad(wgpu_canopyTex, vec2<i32>(_e213, ((2i * _e215) + 1i)), 0i);
    b_7 = _e221;
    let _e223: vec4<u32> = b_7;
    let _e226: i32 = yv_1;
    if (i32(_e223.x) <= _e226) {
        {
            let _e228: vec4<u32> = a_8;
            return (f32(_e228.w) / 255f);
        }
    }
    let _e233: vec4<u32> = b_7;
    let _e236: i32 = yv_1;
    if (i32(_e233.z) <= _e236) {
        {
            let _e238: vec4<u32> = b_7;
            return (f32(_e238.y) / 255f);
        }
    }
    let _e243: vec4<u32> = b_7;
    return (f32(_e243.w) / 255f);
}

fn plantCanopy(v_25: vec4<u32>) -> f32 {
    var v_26: vec4<u32>;

    v_26 = v_25;
    let _e175: vec4<u32> = v_26;
    return (f32((_e175.z & 15u)) / 15f);
}

fn crownPass(v_27: vec4<u32>) -> f32 {
    var v_28: vec4<u32>;

    v_28 = v_27;
    let _e175: vec4<u32> = v_28;
    return (f32((_e175.z >> 4u)) / 15f);
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
    let _e198: vec3<f32> = local_26;
    l_4 = dot(_e198, vec3<f32>(0.2126f, 0.7152f, 0.0722f));
    let _e205: f32 = l_4;
    if (_e205 > 0.001953125f) {
        {
            let _e210: f32 = a_10;
            let _e214: f32 = l_4;
            sum_1 = (clamp(_e210, 0f, 1f) + _e214);
            let _e217: bool = smoothLight;
            if _e217 {
                let _e218: f32 = l_4;
                let _e219: f32 = sum_1;
                local_27 = (_e218 / _e219);
            } else {
                let _e221: f32 = l_4;
                let _e222: f32 = sum_1;
                let _e224: f32 = n_1;
                let _e229: f32 = n_1;
                local_27 = (floor((((_e221 / _e222) * _e224) + 0.5f)) / _e229);
            }
            let _e232: f32 = local_27;
            share = _e232;
            let _e234: vec3<f32> = hue;
            let _e235: vec3<f32> = local_26;
            let _e236: f32 = l_4;
            let _e239: f32 = share;
            hue = mix(_e234, (_e235 / vec3(_e236)), vec3(_e239));
            let _e242: f32 = sum_1;
            a_10 = _e242;
        }
    }
    let _e243: bool = smoothLight;
    if _e243 {
        let _e244: f32 = a_10;
        let _e248: f32 = sun_1;
        local_28 = (clamp(_e244, 0f, 1f) + (_e248 * u.dayK.y));
    } else {
        let _e254: f32 = a_10;
        let _e258: f32 = n_1;
        let _e263: f32 = sun_1;
        let _e269: f32 = n_1;
        local_28 = ((floor(((clamp(_e254, 0f, 1f) * _e258) + 0.5f)) + (_e263 * u.dayK.x)) / _e269);
    }
    let _e272: f32 = local_28;
    t_15 = _e272;
    let _e274: vec3<f32> = hue;
    let _e286: f32 = t_15;
    return (_e274 * (u.lightK.x * (u.lightK.y + ((1f - u.lightK.y) * _e286))));
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
    let _e190: bool = smoothLight_1;
    if _e190 {
        let _e192: f32 = sunOn_1;
        local_29 = (1f + (_e192 * SMOOTH_SUN));
    } else {
        let _e195: f32 = n_2;
        let _e199: f32 = sunOn_1;
        let _e201: f32 = n_2;
        local_29 = ((floor((_e195 + 0.5f)) + _e199) / _e201);
    }
    let _e204: f32 = local_29;
    t_16 = _e204;
    let _e225: f32 = t_16;
    return ((u.ambientC.xyz / vec3(u.dayK.z)) * (u.lightK.x * (u.lightK.y + ((1f - u.lightK.y) * _e225))));
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
    let _e180: vec3<u32> = textureDimensions(wgpu_glowTex, 0i);
    n_3 = (vec3<f32>(vec3<i32>(_e180)) * 4f);
    let _e186: vec3<f32> = p_14;
    let _e187: vec3<f32> = n_3;
    let _e190: vec4<f32> = textureSampleLevel(wgpu_glowTex, wgpu_sampler_0_33_, (_e186 / _e187), 0f);
    return _e190.xyz;
}

fn crownLets(v_29: vec4<u32>, c_14: vec3<i32>) -> bool {
    var v_30: vec4<u32>;
    var c_15: vec3<i32>;
    var h_5: u32;

    v_30 = v_29;
    c_15 = c_14;
    let _e179: vec3<i32> = c_15;
    let _e181: i32 = wrapX(_e179.x);
    let _e182: vec3<i32> = c_15;
    let _e184: vec3<i32> = c_15;
    let _e187: u32 = cellHash(_e181, _e182.y, _e184.z, 32i);
    h_5 = _e187;
    let _e189: u32 = h_5;
    let _e195: vec4<u32> = v_30;
    let _e196: f32 = crownPass(_e195);
    return ((f32((_e189 & 65535u)) / 65536f) < _e196);
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
    let _e183: vec3<f32> = p_16;
    let _e185: vec3<f32> = p_16;
    fx = (_e183.x - (_e185.y * u.figSun.x));
    let _e193: vec3<f32> = p_16;
    let _e195: vec3<f32> = p_16;
    fz = ((_e193.z - (_e195.y * u.figSun.y)) - u.figSun.z);
    let _e207: f32 = fx;
    let _e208: i32 = W;
    let _e210: f32 = fx;
    let _e211: i32 = W;
    let _e221: f32 = fz;
    t_17 = vec2<f32>(((_e207 - (f32(_e208) * floor((_e210 / f32(_e211))))) * u.figMap.x), (_e221 * u.figMap.y));
    let _e228: vec2<f32> = t_17;
    let _e232: vec2<f32> = t_17;
    if ((_e228.y < 0f) || (_e232.y >= u.figMap.w)) {
        {
            return false;
        }
    }
    let _e240: vec2<f32> = t_17;
    let _e250: f32 = textureLoad(wgpu_figSunTex, vec2<i32>(min(_e240, (u.figMap.zw - vec2(1f)))), 0i);
    let _e252: i32 = H;
    top = (f32(_e250) * f32(_e252));
    let _e256: f32 = top;
    let _e257: vec3<f32> = p_16;
    return (_e256 > (_e257.y + 0.25f));
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
    let _e181: vec3<f32> = p_18;
    let _e182: bool = figureShades(_e181);
    if _e182 {
        {
            return 0f;
        }
    }
    L = u.sunK.xyz;
    let _e188: vec3<f32> = L;
    dir = vec3<i32>(sign(_e188));
    let _e192: vec3<f32> = L;
    if (abs(_e192.x) > 0.000001f) {
        let _e198: vec3<f32> = L;
        local_30 = (1f / abs(_e198.x));
    } else {
        local_30 = 1000000000000000000000000000000f;
    }
    let _e204: f32 = local_30;
    let _e205: vec3<f32> = L;
    if (abs(_e205.y) > 0.000001f) {
        let _e211: vec3<f32> = L;
        local_31 = (1f / abs(_e211.y));
    } else {
        local_31 = 1000000000000000000000000000000f;
    }
    let _e217: f32 = local_31;
    let _e218: vec3<f32> = L;
    if (abs(_e218.z) > 0.000001f) {
        let _e224: vec3<f32> = L;
        local_32 = (1f / abs(_e224.z));
    } else {
        local_32 = 1000000000000000000000000000000f;
    }
    let _e230: f32 = local_32;
    inv = vec3<f32>(_e204, _e217, _e230);
    let _e233: vec3<i32> = dir;
    if (_e233.x > 0i) {
        let _e237: vec3<i32> = c_17;
        let _e242: vec3<f32> = p_18;
        local_33 = (f32((_e237.x + 1i)) - _e242.x);
    } else {
        let _e245: vec3<f32> = p_18;
        let _e247: vec3<i32> = c_17;
        local_33 = (_e245.x - f32(_e247.x));
    }
    let _e252: f32 = local_33;
    let _e253: vec3<f32> = inv;
    let _e256: vec3<i32> = dir;
    if (_e256.y > 0i) {
        let _e260: vec3<i32> = c_17;
        let _e265: vec3<f32> = p_18;
        local_34 = (f32((_e260.y + 1i)) - _e265.y);
    } else {
        let _e268: vec3<f32> = p_18;
        let _e270: vec3<i32> = c_17;
        local_34 = (_e268.y - f32(_e270.y));
    }
    let _e275: f32 = local_34;
    let _e276: vec3<f32> = inv;
    let _e279: vec3<i32> = dir;
    if (_e279.z > 0i) {
        let _e283: vec3<i32> = c_17;
        let _e288: vec3<f32> = p_18;
        local_35 = (f32((_e283.z + 1i)) - _e288.z);
    } else {
        let _e291: vec3<f32> = p_18;
        let _e293: vec3<i32> = c_17;
        local_35 = (_e291.z - f32(_e293.z));
    }
    let _e298: f32 = local_35;
    let _e299: vec3<f32> = inv;
    next = vec3<f32>((_e252 * _e253.x), (_e275 * _e276.y), (_e298 * _e299.z));
    loop {
        let _e308: i32 = i_7;
        if !((_e308 < SUN_MARCH)) {
            break;
        }
        {
            let _e314: vec3<i32> = c_17;
            let _e316: i32 = H;
            let _e318: vec3<i32> = c_17;
            let _e323: vec3<i32> = c_17;
            let _e325: i32 = D;
            if (((_e314.y >= _e316) || (_e318.z < 0i)) || (_e323.z >= _e325)) {
                {
                    let _e328: f32 = pass_;
                    return _e328;
                }
            }
            let _e329: vec3<i32> = c_17;
            if (_e329.y < 0i) {
                {
                    return 0f;
                }
            }
            let _e334: vec3<f32> = next;
            let _e336: vec3<f32> = next;
            let _e339: vec3<f32> = next;
            let _e341: vec3<f32> = next;
            if ((_e334.x < _e336.y) && (_e339.x < _e341.z)) {
                local_37 = 0i;
            } else {
                let _e346: vec3<f32> = next;
                let _e348: vec3<f32> = next;
                if (_e346.y < _e348.z) {
                    local_36 = 1i;
                } else {
                    local_36 = 2i;
                }
                let _e354: i32 = local_36;
                local_37 = _e354;
            }
            let _e356: i32 = local_37;
            axis = _e356;
            let _e358: vec3<i32> = c_17;
            let _e360: vec3<i32> = c_17;
            let _e362: vec3<i32> = c_17;
            let _e364: vec4<u32> = at(_e358.x, _e360.y, _e362.z);
            v_31 = _e364;
            let _e366: vec4<u32> = v_31;
            let _e367: bool = solidV(_e366);
            if _e367 {
                {
                    return 0f;
                }
            }
            let _e369: vec4<u32> = v_31;
            let _e370: i32 = partOf(_e369);
            part_4 = _e370;
            if !(PLANT_LIGHT) {
                {
                    let _e373: i32 = part_4;
                    let _e375: i32 = part_4;
                    let _e378: i32 = part_4;
                    let _e380: vec4<u32> = v_31;
                    let _e381: bool = softMark(_e380);
                    if (((_e373 == TRUNK) || (_e375 == LOG)) || ((_e378 == ANIMAL_INTERIM) && !(_e381))) {
                        {
                            return 0f;
                        }
                    }
                    let _e386: i32 = part_4;
                    let _e388: i32 = part_4;
                    let _e391: vec4<u32> = v_31;
                    let _e392: vec3<i32> = c_17;
                    let _e393: bool = crownLets(_e391, _e392);
                    let _e396: vec4<u32> = v_31;
                    let _e397: bool = cordCell(_e396);
                    if ((((_e386 == CROWN) || (_e388 == CROWN_HEART)) && !(_e393)) && !(_e397)) {
                        {
                            return 0f;
                        }
                    }
                }
            } else {
                {
                    let _e401: i32 = part_4;
                    let _e403: i32 = part_4;
                    let _e405: vec4<u32> = v_31;
                    let _e406: bool = softMark(_e405);
                    if ((_e401 == LOG) || ((_e403 == ANIMAL_INTERIM) && !(_e406))) {
                        {
                            return 0f;
                        }
                    }
                    let _e411: i32 = part_4;
                    let _e413: i32 = part_4;
                    let _e415: i32 = part_4;
                    let _e418: vec4<u32> = v_31;
                    let _e419: vec3<i32> = c_17;
                    let _e420: bool = crownLets(_e418, _e419);
                    let _e423: vec4<u32> = v_31;
                    let _e424: bool = cordCell(_e423);
                    if ((_e411 == TRUNK) || ((((_e413 == CROWN) || (_e415 == CROWN_HEART)) && !(_e420)) && !(_e424))) {
                        {
                            let _e428: f32 = pass_;
                            let _e430: vec3<i32> = c_17;
                            let _e432: vec3<i32> = c_17;
                            let _e434: vec3<i32> = c_17;
                            let _e436: f32 = plantSize(_e430.x, _e432.y, _e434.z);
                            pass_ = (_e428 * (1f - _e436));
                            let _e439: f32 = pass_;
                            if (_e439 <= 0f) {
                                {
                                    return 0f;
                                }
                            }
                        }
                    }
                }
            }
            let _e443: i32 = axis;
            if (_e443 == 0i) {
                {
                    let _e447: vec3<i32> = c_17;
                    let _e449: vec3<i32> = dir;
                    c_17.x = (_e447.x + _e449.x);
                    let _e453: vec3<f32> = next;
                    let _e455: vec3<f32> = inv;
                    next.x = (_e453.x + _e455.x);
                }
            } else {
                let _e458: i32 = axis;
                if (_e458 == 1i) {
                    {
                        let _e462: vec3<i32> = c_17;
                        let _e464: vec3<i32> = dir;
                        c_17.y = (_e462.y + _e464.y);
                        let _e468: vec3<f32> = next;
                        let _e470: vec3<f32> = inv;
                        next.y = (_e468.y + _e470.y);
                    }
                } else {
                    {
                        let _e474: vec3<i32> = c_17;
                        let _e476: vec3<i32> = dir;
                        c_17.z = (_e474.z + _e476.z);
                        let _e480: vec3<f32> = next;
                        let _e482: vec3<f32> = inv;
                        next.z = (_e480.z + _e482.z);
                    }
                }
            }
        }
        continuing {
            let _e311: i32 = i_7;
            i_7 = (_e311 + 1i);
        }
    }
    let _e485: f32 = pass_;
    return _e485;
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
    let _e185: vec3<f32> = base_3;
    let _e186: f32 = ambient_1;
    let _e187: f32 = sun_3;
    let _e188: vec3<f32> = local_39;
    let _e189: vec3<f32> = ladderLight(_e186, _e187, _e188);
    c_18 = (_e185 * _e189);
    let _e192: vec3<f32> = c_18;
    let _e199: f32 = sun_3;
    let _e201: f32 = ndl_1;
    return mix(_e192, u.sunLean.xyz, vec3(clamp(((u.sunK.w * _e199) * _e201), 0f, 1f)));
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
    let _e198: i32 = x_55;
    let _e200: i32 = dx_21;
    let _e204: i32 = S;
    let _e208: i32 = y_47;
    let _e212: i32 = dy_15;
    let _e216: i32 = S;
    let _e220: i32 = z_55;
    p_19 = vec3<f32>((f32(_e198) + ((f32(_e200) + 0.5f) / f32(_e204))), (f32((_e208 + 1i)) - ((f32(_e212) + 0.5f) / f32(_e216))), f32(_e220));
    let _e224: f32 = ndl_2;
    if ((_e224 > 0f) && (u.sunK.y > 0f)) {
        {
            let _e233: vec3<f32> = p_19;
            let _e234: i32 = x_55;
            let _e235: i32 = y_47;
            let _e236: i32 = z_55;
            let _e240: f32 = sunReaches(_e233, vec3<i32>(_e234, _e235, (_e236 - 1i)));
            sun_4 = _e240;
        }
    }
    let _e241: vec3<f32> = p_19;
    let _e247: vec3<f32> = glowAt((_e241 - vec3<f32>(0f, 0f, 0.5f)));
    local_40 = _e247;
    let _e249: vec3<f32> = base_5;
    let _e250: i32 = x_55;
    let _e251: i32 = y_47;
    let _e252: i32 = z_55;
    let _e255: f32 = skyOpen(_e250, _e251, (_e252 - 1i));
    let _e256: i32 = x_55;
    let _e257: i32 = y_47;
    let _e258: i32 = z_55;
    let _e259: i32 = dx_21;
    let _e260: i32 = dy_15;
    let _e261: f32 = aoFront(_e256, _e257, _e258, _e259, _e260);
    let _e263: f32 = canopy_1;
    let _e265: f32 = sun_4;
    let _e266: f32 = ndl_2;
    let _e267: vec3<f32> = local_40;
    let _e268: vec3<f32> = litBy(_e249, ((_e255 * _e261) * _e263), _e265, _e266, _e267);
    return _e268;
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
    let _e197: i32 = x_57;
    let _e199: i32 = dx_23;
    let _e203: i32 = S;
    let _e207: i32 = y_49;
    let _e211: i32 = z_57;
    let _e215: i32 = dy_17;
    let _e219: i32 = RISE;
    p_20 = vec3<f32>((f32(_e197) + ((f32(_e199) + 0.5f) / f32(_e203))), f32((_e207 + 1i)), (f32((_e211 + 1i)) - ((f32(_e215) + 0.5f) / f32(_e219))));
    let _e225: f32 = ndl_3;
    if (_e225 > 0f) {
        {
            let _e228: vec3<f32> = p_20;
            let _e229: i32 = x_57;
            let _e230: i32 = y_49;
            let _e233: i32 = z_57;
            let _e235: f32 = sunReaches(_e228, vec3<i32>(_e229, (_e230 + 1i), _e233));
            sun_5 = _e235;
        }
    }
    let _e236: vec3<f32> = p_20;
    let _e242: vec3<f32> = glowAt((_e236 + vec3<f32>(0f, 0.5f, 0f)));
    local_41 = _e242;
    let _e244: vec3<f32> = base_7;
    let _e245: i32 = x_57;
    let _e246: i32 = y_49;
    let _e249: i32 = z_57;
    let _e250: f32 = skyOpen(_e245, (_e246 + 1i), _e249);
    let _e251: i32 = x_57;
    let _e252: i32 = y_49;
    let _e253: i32 = z_57;
    let _e254: i32 = dx_23;
    let _e255: i32 = dy_17;
    let _e256: f32 = aoTop(_e251, _e252, _e253, _e254, _e255);
    let _e258: f32 = canopy_3;
    let _e260: f32 = sun_5;
    let _e261: f32 = ndl_3;
    let _e262: vec3<f32> = local_41;
    let _e263: vec3<f32> = litBy(_e244, ((_e250 * _e256) * _e258), _e260, _e261, _e262);
    return _e263;
}

fn boneRow(t_18: Tile, bits: u32) -> i32 {
    var t_19: Tile;
    var bits_1: u32;
    var local_42: i32;

    t_19 = t_18;
    bits_1 = bits;
    let _e192: Tile = t_19;
    if (_e192.rig.x < 0i) {
        local_42 = -1i;
    } else {
        let _e200: Tile = t_19;
        let _e203: u32 = bits_1;
        local_42 = (3i * (_e200.rig.x + i32(((_e203 >> 14u) & 63u))));
    }
    let _e213: i32 = local_42;
    return _e213;
}

fn boneMove(row_5: i32, p_21: vec3<f32>) -> vec3<f32> {
    var row_6: i32;
    var p_22: vec3<f32>;
    var q_2: vec4<f32>;

    row_6 = row_5;
    p_22 = p_21;
    let _e192: vec3<f32> = p_22;
    q_2 = vec4<f32>(_e192.x, _e192.y, _e192.z, 1f);
    let _e199: i32 = row_6;
    let _e201: vec4<f32> = global_4.bones[_e199];
    let _e202: vec4<f32> = q_2;
    let _e204: i32 = row_6;
    let _e208: vec4<f32> = global_4.bones[(_e204 + 1i)];
    let _e209: vec4<f32> = q_2;
    let _e211: i32 = row_6;
    let _e215: vec4<f32> = global_4.bones[(_e211 + 2i)];
    let _e216: vec4<f32> = q_2;
    return vec3<f32>(dot(_e201, _e202), dot(_e208, _e209), dot(_e215, _e216));
}

fn boneTurn(row_7: i32, v_32: vec3<f32>) -> vec3<f32> {
    var row_8: i32;
    var v_33: vec3<f32>;

    row_8 = row_7;
    v_33 = v_32;
    let _e192: i32 = row_8;
    let _e194: vec4<f32> = global_4.bones[_e192];
    let _e196: vec3<f32> = v_33;
    let _e198: i32 = row_8;
    let _e202: vec4<f32> = global_4.bones[(_e198 + 1i)];
    let _e204: vec3<f32> = v_33;
    let _e206: i32 = row_8;
    let _e210: vec4<f32> = global_4.bones[(_e206 + 2i)];
    let _e212: vec3<f32> = v_33;
    return vec3<f32>(dot(_e194.xyz, _e196), dot(_e202.xyz, _e204), dot(_e210.xyz, _e212));
}

fn bonePose(row_9: i32, p_23: vec3<f32>) -> vec3<f32> {
    var row_10: i32;
    var p_24: vec3<f32>;
    var local_43: vec3<f32>;

    row_10 = row_9;
    p_24 = p_23;
    let _e192: i32 = row_10;
    if (_e192 < 0i) {
        let _e195: vec3<f32> = p_24;
        local_43 = _e195;
    } else {
        let _e196: i32 = row_10;
        let _e197: vec3<f32> = p_24;
        let _e198: vec3<f32> = boneMove(_e196, _e197);
        local_43 = _e198;
    }
    let _e200: vec3<f32> = local_43;
    return _e200;
}

fn litFace(base_8: vec3<f32>, p_25: vec3<f32>, n_4: vec3<f32>, ao: f32) -> vec3<f32> {
    var base_9: vec3<f32>;
    var p_26: vec3<f32>;
    var n_5: vec3<f32>;
    var ao_1: f32;
    var c_19: vec3<i32>;
    var ndl_4: f32;
    var sun_6: f32 = 0f;
    var a_11: f32;

    base_9 = base_8;
    p_26 = p_25;
    n_5 = n_4;
    ao_1 = ao;
    let _e213: vec3<f32> = p_26;
    let _e214: vec3<f32> = n_5;
    c_19 = vec3<i32>(floor((_e213 + (_e214 * 0.5f))));
    let _e221: vec3<f32> = n_5;
    ndl_4 = max(dot(_e221, u.sunK.xyz), 0f);
    let _e231: f32 = ndl_4;
    if ((_e231 > 0f) && (u.sunK.y > 0f)) {
        {
            let _e240: vec3<f32> = p_26;
            let _e241: vec3<f32> = n_5;
            let _e245: vec3<i32> = c_19;
            let _e246: f32 = sunReaches((_e240 + (_e241 * 0.01f)), _e245);
            sun_6 = _e246;
        }
    }
    let _e247: vec3<i32> = c_19;
    let _e249: vec3<i32> = c_19;
    let _e251: vec3<i32> = c_19;
    let _e253: f32 = skyOpen(_e247.x, _e249.y, _e251.z);
    let _e254: f32 = ao_1;
    let _e256: vec3<i32> = c_19;
    let _e258: vec3<i32> = c_19;
    let _e260: vec3<f32> = p_26;
    let _e262: vec3<f32> = n_5;
    let _e269: f32 = canopyAt(_e256.x, _e258.z, i32(floor((_e260.y - (_e262.y * 0.01f)))));
    a_11 = ((_e253 * _e254) * _e269);
    let _e272: vec3<f32> = base_9;
    let _e273: f32 = a_11;
    let _e274: f32 = sun_6;
    let _e275: f32 = ndl_4;
    let _e276: vec3<f32> = p_26;
    let _e277: vec3<f32> = n_5;
    let _e281: vec3<f32> = glowAt((_e276 + (_e277 * 0.5f)));
    let _e282: vec3<f32> = litBy(_e272, _e273, _e274, _e275, _e281);
    return _e282;
}

fn flatTop(base_10: vec3<f32>, p_27: vec3<f32>) -> vec3<f32> {
    var base_11: vec3<f32>;
    var p_28: vec3<f32>;
    var c_20: vec3<i32>;
    var local_44: i32;
    var gap_2: i32;

    base_11 = base_10;
    p_28 = p_27;
    let _e209: vec3<f32> = p_28;
    c_20 = vec3<i32>(floor((_e209 - vec3<f32>(0f, 0.01f, 0f))));
    let _e218: vec3<i32> = c_20;
    let _e220: bool = inY(_e218.y);
    if _e220 {
        let _e221: vec3<i32> = c_20;
        let _e223: vec3<i32> = c_20;
        let _e225: vec3<i32> = c_20;
        let _e227: i32 = roofGap(_e221.x, _e223.y, _e225.z);
        local_44 = _e227;
    } else {
        local_44 = 0i;
    }
    let _e230: i32 = local_44;
    gap_2 = _e230;
    let _e232: vec3<f32> = base_11;
    let _e233: i32 = gap_2;
    let _e234: f32 = roofShade(_e233);
    let _e235: vec3<f32> = plantLit(_e232, _e234);
    return _e235;
}

fn main_1() {
    var n_6: vec3<f32>;
    var p_29: vec3<f32>;
    var top_1: bool;
    var openUp: bool;
    var entry: i32;
    var glyph_2: i32;
    var hz: f32;
    var face: i32;
    var cap_1: bool;
    var local_45: i32;
    var rows_2: i32;
    var dx_24: i32;
    var dy_18: i32;
    var local_46: i32;
    var stampGlyph: i32;
    var local_47: i32;
    var q_3: u32;
    var local_48: i32;
    var pigment: i32;
    var tone: i32;
    var aoTexel: vec2<i32>;
    var ao_2: f32;
    var capTexel: vec2<i32>;
    var capAo: f32;
    var pe: Pal;
    var base_12: vec3<f32>;
    var drawn_2: vec3<f32>;
    var c_21: vec3<f32>;
    var up_4: vec3<f32>;
    var gain: f32;
    var gain_1: f32;
    var o_3: vec3<f32>;

    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;
    let _e223: vec3<f32> = vNormal_1;
    n_6 = normalize(_e223);
    let _e226: vec3<f32> = vWorld_1;
    p_29 = _e226;
    let _e228: vec4<i32> = vInfo_1;
    let _e234: vec4<i32> = vInfo_1;
    top_1 = (((_e228.w & 7i) == 3i) || ((_e234.w & 7i) == 2i));
    let _e242: vec4<i32> = vInfo_1;
    openUp = (_e242.z != 0i);
    let _e247: vec4<i32> = vInfo_1;
    entry = _e247.x;
    let _e250: vec4<i32> = vInfo_1;
    glyph_2 = _e250.y;
    let _e253: vec3<f32> = p_29;
    let _e255: f32 = hazeAt(_e253.z);
    hz = _e255;
    let _e257: vec3<f32> = p_29;
    outZ = _e257.z;
    let _e259: vec4<i32> = vInfo_1;
    face = (_e259.w & 7i);
    let _e264: i32 = face;
    let _e267: i32 = face;
    cap_1 = ((_e264 == 2i) || (_e267 == 3i));
    let _e272: bool = cap_1;
    if _e272 {
        let _e273: i32 = RISE;
        local_45 = _e273;
    } else {
        let _e274: i32 = S;
        local_45 = _e274;
    }
    let _e276: i32 = local_45;
    rows_2 = _e276;
    let _e278: vec2<f32> = vFace_1;
    let _e280: i32 = S;
    let _e285: i32 = S;
    dx_24 = clamp(i32((_e278.x * f32(_e280))), 0i, (_e285 - 1i));
    let _e290: vec2<f32> = vFace_1;
    let _e292: i32 = rows_2;
    let _e297: i32 = rows_2;
    dy_18 = clamp(i32((_e290.y * f32(_e292))), 0i, (_e297 - 1i));
    let _e302: i32 = glyph_2;
    if (_e302 == 0i) {
        local_46 = 1i;
    } else {
        let _e306: i32 = glyph_2;
        local_46 = _e306;
    }
    let _e308: i32 = local_46;
    stampGlyph = _e308;
    let _e310: i32 = stampGlyph;
    let _e311: i32 = dx_24;
    let _e312: bool = cap_1;
    if _e312 {
        let _e313: i32 = S;
        local_47 = _e313;
    } else {
        local_47 = 0i;
    }
    let _e316: i32 = local_47;
    let _e317: i32 = dy_18;
    let _e319: u32 = glyphTexel(ANIMAL_INTERIM, _e310, _e311, (_e316 + _e317));
    q_3 = _e319;
    let _e321: i32 = glyph_2;
    if (_e321 == 0i) {
        local_48 = 0i;
    } else {
        let _e325: u32 = q_3;
        local_48 = i32((_e325 & 3u));
    }
    let _e330: i32 = local_48;
    pigment = _e330;
    let _e332: u32 = q_3;
    tone = i32((_e332 >> 2u));
    let _e338: i32 = tone;
    if (_e338 == 63i) {
        {
            discard;
        }
    }
    let _e341: vec2<f32> = vFace_1;
    let _e342: vec2<i32> = vFaceSize_1;
    let _e348: vec2<i32> = vFaceSize_1;
    aoTexel = clamp(vec2<i32>((_e341 * vec2<f32>(_e342))), vec2(0i), (_e348 - vec2(1i)));
    let _e354: vec4<i32> = vInfo_1;
    let _e362: vec2<i32> = aoTexel;
    let _e364: vec2<i32> = aoTexel;
    let _e366: vec2<i32> = vFaceSize_1;
    let _e367: f32 = modelCrease(((u32(_e354.w) >> 3u) & 255u), _e362.x, _e364.y, _e366);
    ao_2 = _e367;
    let _e369: vec2<f32> = vRimFace_1;
    let _e370: vec2<i32> = vCapSize_1;
    let _e376: vec2<i32> = vCapSize_1;
    capTexel = clamp(vec2<i32>((_e369 * vec2<f32>(_e370))), vec2(0i), (_e376 - vec2(1i)));
    let _e382: vec4<i32> = vInfo_1;
    let _e390: vec2<i32> = capTexel;
    let _e392: vec2<i32> = capTexel;
    let _e394: vec2<i32> = vCapSize_1;
    let _e395: f32 = modelCrease(((u32(_e382.w) >> 11u) & 255u), _e390.x, _e392.y, _e394);
    capAo = _e395;
    let _e397: i32 = entry;
    let _e398: i32 = pigment;
    let _e401: Pal = global_5.pal[(_e397 + _e398)];
    pe = _e401;
    let _e403: Pal = pe;
    base_12 = _e403.base.xyz;
    let _e407: Pal = pe;
    let _e412: i32 = tone;
    if (LIT && ((_e407.base.w > 0.5f) || (_e412 == TONE_EMIT))) {
        {
            let _e416: Pal = pe;
            if any((_e416.emit.xyz > vec3(0f))) {
                {
                    let _e423: Pal = pe;
                    let _e426: f32 = hz;
                    let _e427: vec3<f32> = hazed(_e423.emit.xyz, _e426);
                    drawn_2 = _e427;
                    let _e429: vec3<f32> = drawn_2;
                    outColour = vec4<f32>(_e429.x, _e429.y, _e429.z, 1f);
                    let _e435: vec3<f32> = drawn_2;
                    let _e436: f32 = hz;
                    let _e447: vec3<f32> = max((_e435 - (clamp(_e436, 0f, 1f) * u.hazeC.xyz)), vec3(0f));
                    outEmit = vec4<f32>(_e447.x, _e447.y, _e447.z, 1f);
                    return;
                }
            }
        }
    }
    outEmit = vec4<f32>(0f, 0f, 0f, 1f);
    if LIT {
        {
            let _e459: FigurePush = pc;
            let _e463: Tile = global_2.tiles[_e459.k.x];
            up_4 = _e463.ay.xyz;
            let _e467: bool = top_1;
            if _e467 {
                {
                    let _e468: vec3<f32> = base_12;
                    let _e469: vec3<f32> = p_29;
                    let _e470: vec3<f32> = n_6;
                    let _e471: f32 = ao_2;
                    let _e472: vec3<f32> = litFace(_e468, _e469, _e470, _e471);
                    c_21 = _e472;
                    let _e473: i32 = tone;
                    if (_e473 == 1i) {
                        {
                            let _e476: vec3<f32> = c_21;
                            c_21 = (_e476 * u.plantA.w);
                        }
                    } else {
                        let _e481: i32 = tone;
                        let _e484: i32 = tone;
                        if ((_e481 >= 32i) && (_e484 <= 47i)) {
                            {
                                let _e489: i32 = tone;
                                gain = (0.5f + (f32((_e489 & 15i)) / 16f));
                                let _e497: vec3<f32> = c_21;
                                let _e498: vec3<f32> = c_21;
                                let _e504: f32 = gain;
                                c_21 = mix(_e497, ((_e498 / vec3(u.plantA.x)) * _e504), vec3(u.shadeB.y));
                            }
                        }
                    }
                }
            } else {
                let _e511: i32 = tone;
                if (_e511 == 3i) {
                    {
                        let _e514: vec3<f32> = base_12;
                        let _e515: vec3<f32> = vRimWorld_1;
                        let _e516: vec3<f32> = up_4;
                        let _e517: f32 = capAo;
                        let _e518: vec3<f32> = litFace(_e514, _e515, _e516, _e517);
                        c_21 = _e518;
                    }
                } else {
                    {
                        let _e519: i32 = tone;
                        if (_e519 == 1i) {
                            {
                                let _e522: vec3<f32> = base_12;
                                base_12 = (_e522 * u.shadeB.x);
                            }
                        } else {
                            let _e527: i32 = tone;
                            let _e530: i32 = tone;
                            if ((_e527 == 4i) || (_e530 == 5i)) {
                                {
                                    let _e534: vec3<f32> = base_12;
                                    let _e535: i32 = entry;
                                    let _e537: Pal = global_5.pal[_e535];
                                    base_12 = mix(_e534, _e537.base.xyz, vec3(u.plantB.x));
                                    let _e545: i32 = tone;
                                    if (_e545 == 5i) {
                                        {
                                            let _e548: vec3<f32> = base_12;
                                            base_12 = (_e548 * u.plantA.w);
                                        }
                                    }
                                }
                            } else {
                                let _e553: i32 = tone;
                                let _e556: i32 = tone;
                                if ((_e553 >= 16i) && (_e556 <= 31i)) {
                                    {
                                        let _e560: vec3<f32> = base_12;
                                        let _e562: i32 = tone;
                                        base_12 = (_e560 * (0.5f + (f32((_e562 & 15i)) / 16f)));
                                    }
                                }
                            }
                        }
                        let _e570: vec3<f32> = base_12;
                        let _e571: vec3<f32> = p_29;
                        let _e572: vec3<f32> = n_6;
                        let _e573: f32 = ao_2;
                        let _e574: vec3<f32> = litFace(_e570, _e571, _e572, _e573);
                        c_21 = _e574;
                        let _e575: i32 = tone;
                        let _e578: bool = openUp;
                        if ((_e575 == 2i) && _e578) {
                            {
                                let _e580: vec3<f32> = c_21;
                                let _e581: vec3<f32> = base_12;
                                let _e582: vec3<f32> = vRimWorld_1;
                                let _e583: vec3<f32> = up_4;
                                let _e584: f32 = capAo;
                                let _e585: vec3<f32> = litFace(_e581, _e582, _e583, _e584);
                                c_21 = mix(_e580, _e585, vec3(u.plantA.z));
                            }
                        }
                    }
                }
            }
        }
    } else {
        {
            let _e591: bool = top_1;
            if _e591 {
                {
                    let _e592: vec3<f32> = base_12;
                    let _e593: vec3<f32> = p_29;
                    let _e594: vec3<f32> = flatTop(_e592, _e593);
                    c_21 = _e594;
                    let _e595: i32 = tone;
                    if (_e595 == 1i) {
                        {
                            let _e598: vec3<f32> = c_21;
                            c_21 = (_e598 * u.plantA.w);
                        }
                    } else {
                        let _e603: i32 = tone;
                        let _e606: i32 = tone;
                        if ((_e603 >= 32i) && (_e606 <= 47i)) {
                            {
                                let _e611: i32 = tone;
                                gain_1 = (0.5f + (f32((_e611 & 15i)) / 16f));
                                let _e619: vec3<f32> = c_21;
                                let _e620: vec3<f32> = base_12;
                                let _e621: f32 = gain_1;
                                c_21 = mix(_e619, (_e620 * _e621), vec3(u.shadeB.y));
                            }
                        }
                    }
                }
            } else {
                let _e628: i32 = tone;
                if (_e628 == 3i) {
                    {
                        let _e631: vec3<f32> = base_12;
                        let _e632: vec3<f32> = p_29;
                        let _e633: vec3<f32> = flatTop(_e631, _e632);
                        c_21 = _e633;
                    }
                } else {
                    {
                        let _e634: i32 = tone;
                        if (_e634 == 1i) {
                            {
                                let _e637: vec3<f32> = base_12;
                                base_12 = (_e637 * u.shadeB.x);
                            }
                        } else {
                            let _e642: i32 = tone;
                            let _e645: bool = openUp;
                            if ((_e642 == 2i) && _e645) {
                                {
                                    let _e647: vec3<f32> = base_12;
                                    let _e648: vec3<f32> = base_12;
                                    let _e649: vec3<f32> = p_29;
                                    let _e650: vec3<f32> = flatTop(_e648, _e649);
                                    base_12 = mix(_e647, _e650, vec3(u.plantA.z));
                                }
                            } else {
                                let _e656: i32 = tone;
                                let _e659: i32 = tone;
                                if ((_e656 == 4i) || (_e659 == 5i)) {
                                    {
                                        let _e663: vec3<f32> = base_12;
                                        let _e664: i32 = entry;
                                        let _e666: Pal = global_5.pal[_e664];
                                        base_12 = mix(_e663, _e666.base.xyz, vec3(u.plantB.x));
                                        let _e674: i32 = tone;
                                        if (_e674 == 5i) {
                                            {
                                                let _e677: vec3<f32> = base_12;
                                                base_12 = (_e677 * u.plantA.w);
                                            }
                                        }
                                    }
                                } else {
                                    let _e682: i32 = tone;
                                    let _e685: i32 = tone;
                                    if ((_e682 >= 16i) && (_e685 <= 31i)) {
                                        {
                                            let _e689: vec3<f32> = base_12;
                                            let _e691: i32 = tone;
                                            base_12 = (_e689 * (0.5f + (f32((_e691 & 15i)) / 16f)));
                                        }
                                    }
                                }
                            }
                        }
                        let _e699: vec3<f32> = base_12;
                        c_21 = _e699;
                    }
                }
            }
        }
    }
    let _e700: vec3<f32> = c_21;
    let _e701: f32 = hz;
    let _e702: vec3<f32> = hazed(_e700, _e701);
    o_3 = _e702;
    if !(LIT) {
        {
            let _e705: vec3<f32> = o_3;
            o_3 = (_e705 * u.tint.xyz);
        }
    }
    let _e710: vec3<f32> = o_3;
    outColour = vec4<f32>(_e710.x, _e710.y, _e710.z, 1f);
    return;
}

@fragment 
fn main(@location(0) vWorld: vec3<f32>, @location(1) vNormal: vec3<f32>, @location(2) vFace: vec2<f32>, @location(3) @interpolate(flat) vInfo: vec4<i32>, @location(4) @interpolate(flat) vFaceSize: vec2<i32>, @location(5) vRimWorld: vec3<f32>, @location(6) vRimFace: vec2<f32>, @location(7) @interpolate(flat) vCapSize: vec2<i32>) -> FragmentOutput {
    vWorld_1 = vWorld;
    vNormal_1 = vNormal;
    vFace_1 = vFace;
    vInfo_1 = vInfo;
    vFaceSize_1 = vFaceSize;
    vRimWorld_1 = vRimWorld;
    vRimFace_1 = vRimFace;
    vCapSize_1 = vCapSize;
    main_1();
    let _e289: vec4<f32> = outColour;
    let _e291: vec4<f32> = outEmit;
    let _e293: f32 = outZ;
    return FragmentOutput(_e289, _e291, _e293);
}
