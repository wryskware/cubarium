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

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
    @location(1) outEmit: vec4<f32>,
    @location(2) figureWeatherSurface: vec4<f32>,
    @builtin(frag_depth) gl_FragDepth: f32,
}

const FACE_O: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, 0f), vec3<f32>(1f, 0f, 1f));
const FACE_U: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(-1f, 0f, 0f));
const FACE_V: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f));
const FACE_N: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(-1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, -1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f));

@group(2) @binding(32) 
var wgpu_sampler_2_32_: sampler;
var<immediate> pc: FigurePush;
@group(1) @binding(0) 
var<storage> global: FigureVoxels;
@group(1) @binding(1) 
var<storage> global_1: FigureTiles;
@group(1) @binding(2) 
var<storage> global_2: FigureFigs;
@group(1) @binding(4) 
var<storage> global_3: FigureBones;
@group(2) @binding(0) 
var wgpu_atlasColour: texture_2d<f32>;
@group(2) @binding(1) 
var wgpu_atlasEmit: texture_2d<f32>;
@group(2) @binding(2) 
var wgpu_atlasZ: texture_2d<f32>;
@group(2) @binding(3) 
var wgpu_walkDepth: texture_2d<f32>;
var<private> vFig_1: i32;
var<private> outColour: vec4<f32>;
var<private> outEmit: vec4<f32>;
var<private> figureWeatherSurface: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;
var<private> gl_FragDepth: f32;

fn boneRow(t: Tile, bits: u32) -> i32 {
    var t_1: Tile;
    var bits_1: u32;
    var local: i32;

    t_1 = t;
    bits_1 = bits;
    let _e18: Tile = t_1;
    if (_e18.rig.x < 0i) {
        local = -1i;
    } else {
        let _e26: Tile = t_1;
        let _e29: u32 = bits_1;
        local = (3i * (_e26.rig.x + i32(((_e29 >> 14u) & 63u))));
    }
    let _e39: i32 = local;
    return _e39;
}

fn boneMove(row: i32, p: vec3<f32>) -> vec3<f32> {
    var row_1: i32;
    var p_1: vec3<f32>;
    var q: vec4<f32>;

    row_1 = row;
    p_1 = p;
    let _e18: vec3<f32> = p_1;
    q = vec4<f32>(_e18.x, _e18.y, _e18.z, 1f);
    let _e25: i32 = row_1;
    let _e27: vec4<f32> = global_3.bones[_e25];
    let _e28: vec4<f32> = q;
    let _e30: i32 = row_1;
    let _e34: vec4<f32> = global_3.bones[(_e30 + 1i)];
    let _e35: vec4<f32> = q;
    let _e37: i32 = row_1;
    let _e41: vec4<f32> = global_3.bones[(_e37 + 2i)];
    let _e42: vec4<f32> = q;
    return vec3<f32>(dot(_e27, _e28), dot(_e34, _e35), dot(_e41, _e42));
}

fn boneTurn(row_2: i32, v: vec3<f32>) -> vec3<f32> {
    var row_3: i32;
    var v_1: vec3<f32>;

    row_3 = row_2;
    v_1 = v;
    let _e18: i32 = row_3;
    let _e20: vec4<f32> = global_3.bones[_e18];
    let _e22: vec3<f32> = v_1;
    let _e24: i32 = row_3;
    let _e28: vec4<f32> = global_3.bones[(_e24 + 1i)];
    let _e30: vec3<f32> = v_1;
    let _e32: i32 = row_3;
    let _e36: vec4<f32> = global_3.bones[(_e32 + 2i)];
    let _e38: vec3<f32> = v_1;
    return vec3<f32>(dot(_e20.xyz, _e22), dot(_e28.xyz, _e30), dot(_e36.xyz, _e38));
}

fn bonePose(row_4: i32, p_2: vec3<f32>) -> vec3<f32> {
    var row_5: i32;
    var p_3: vec3<f32>;
    var local_1: vec3<f32>;

    row_5 = row_4;
    p_3 = p_2;
    let _e18: i32 = row_5;
    if (_e18 < 0i) {
        let _e21: vec3<f32> = p_3;
        local_1 = _e21;
    } else {
        let _e22: i32 = row_5;
        let _e23: vec3<f32> = p_3;
        let _e24: vec3<f32> = boneMove(_e22, _e23);
        local_1 = _e24;
    }
    let _e26: vec3<f32> = local_1;
    return _e26;
}

fn tileOverOne(t_2: Tile, tc: vec2<f32>, wz: f32, weight: f32, col: ptr<function, vec4<f32>>, em: ptr<function, vec3<f32>>, zNear: ptr<function, f32>) {
    var t_3: Tile;
    var tc_1: vec2<f32>;
    var wz_1: f32;
    var weight_1: f32;
    var g: vec2<f32>;
    var i0_: vec2<i32>;
    var f: vec2<f32>;
    var j: i32 = 0i;
    var o: vec2<i32>;
    var ti: vec2<i32>;
    var local_2: f32;
    var local_3: f32;
    var w: f32;
    var at: vec2<i32>;
    var c: vec4<f32>;
    var tz: f32;

    t_3 = t_2;
    tc_1 = tc;
    wz_1 = wz;
    weight_1 = weight;
    let _e37: vec2<f32> = tc_1;
    g = (_e37 - vec2(0.5f));
    let _e42: vec2<f32> = g;
    i0_ = vec2<i32>(floor(_e42));
    let _e46: vec2<f32> = g;
    let _e47: vec2<i32> = i0_;
    f = (_e46 - vec2<f32>(_e47));
    loop {
        let _e53: i32 = j;
        if !((_e53 < 4i)) {
            break;
        }
        {
            let _e60: i32 = j;
            let _e63: i32 = j;
            o = vec2<i32>((_e60 & 1i), (_e63 >> 1u));
            let _e69: vec2<i32> = i0_;
            let _e70: vec2<i32> = o;
            ti = (_e69 + _e70);
            let _e73: vec2<i32> = ti;
            let _e78: vec2<i32> = ti;
            let _e79: Tile = t_3;
            if (any((_e73 < vec2(0i))) || any((_e78 >= _e79.rect.zw))) {
                {
                    continue;
                }
            }
            let _e85: vec2<i32> = o;
            if (_e85.x == 1i) {
                let _e89: vec2<f32> = f;
                local_2 = _e89.x;
            } else {
                let _e92: vec2<f32> = f;
                local_2 = (1f - _e92.x);
            }
            let _e96: f32 = local_2;
            let _e97: vec2<i32> = o;
            if (_e97.y == 1i) {
                let _e101: vec2<f32> = f;
                local_3 = _e101.y;
            } else {
                let _e104: vec2<f32> = f;
                local_3 = (1f - _e104.y);
            }
            let _e108: f32 = local_3;
            let _e110: f32 = weight_1;
            w = ((_e96 * _e108) * _e110);
            let _e113: f32 = w;
            if (_e113 <= 0f) {
                {
                    continue;
                }
            }
            let _e116: Tile = t_3;
            let _e119: vec2<i32> = ti;
            at = (_e116.rect.xy + _e119);
            let _e122: vec2<i32> = at;
            let _e124: vec4<f32> = textureLoad(wgpu_atlasColour, _e122, 0i);
            c = _e124;
            let _e126: vec4<f32> = c;
            if (_e126.w < 0.5f) {
                {
                    continue;
                }
            }
            let _e130: vec2<i32> = at;
            let _e132: vec4<f32> = textureLoad(wgpu_atlasZ, _e130, 0i);
            tz = _e132.x;
            let _e135: f32 = tz;
            let _e136: f32 = wz_1;
            let _e137: FigurePush = pc;
            if (_e135 > (_e136 + _e137.atlas.z)) {
                {
                    continue;
                }
            }
            let _e142: f32 = (*zNear);
            let _e143: f32 = tz;
            (*zNear) = min(_e142, _e143);
            let _e145: vec4<f32> = (*col);
            let _e146: f32 = w;
            let _e147: vec4<f32> = c;
            let _e148: vec3<f32> = _e147.xyz;
            (*col) = (_e145 + (_e146 * vec4<f32>(_e148.x, _e148.y, _e148.z, 1f)));
            let _e156: vec3<f32> = (*em);
            let _e157: f32 = w;
            let _e158: vec2<i32> = at;
            let _e160: vec4<f32> = textureLoad(wgpu_atlasEmit, _e158, 0i);
            (*em) = (_e156 + (_e157 * _e160.xyz));
        }
        continuing {
            let _e57: i32 = j;
            j = (_e57 + 1i);
        }
    }
    return;
}

fn tileOver(t_4: Tile, tc_2: vec2<f32>, wz_2: f32, weight_2: f32, col_1: ptr<function, vec4<f32>>, em_1: ptr<function, vec3<f32>>, zNear_1: ptr<function, f32>) {
    var t_5: Tile;
    var tc_3: vec2<f32>;
    var wz_3: f32;
    var weight_3: f32;
    var aa: i32;
    var scale: f32;
    var lo: vec2<f32>;
    var hi: vec2<f32>;
    var i0_1: vec2<i32>;
    var y: i32 = 0i;
    var x: i32;
    var ti_1: vec2<i32>;
    var area: vec2<f32>;
    var w_1: f32;
    var at_1: vec2<i32>;
    var c_1: vec4<f32>;
    var tz_1: f32;

    t_5 = t_4;
    tc_3 = tc_2;
    wz_3 = wz_2;
    weight_3 = weight_2;
    let _e37: Tile = t_5;
    aa = _e37.faceSize.w;
    let _e41: i32 = aa;
    if (_e41 == 1i) {
        {
            let _e44: Tile = t_5;
            let _e45: vec2<f32> = tc_3;
            let _e46: f32 = wz_3;
            let _e47: f32 = weight_3;
            tileOverOne(_e44, _e45, _e46, _e47, col_1, em_1, zNear_1);
            return;
        }
    }
    let _e54: i32 = aa;
    scale = f32(_e54);
    let _e57: vec2<f32> = tc_3;
    let _e61: f32 = scale;
    lo = ((_e57 - vec2(0.5f)) * _e61);
    let _e64: vec2<f32> = lo;
    let _e65: f32 = scale;
    hi = (_e64 + vec2(_e65));
    let _e69: vec2<f32> = lo;
    i0_1 = vec2<i32>(floor(_e69));
    loop {
        let _e75: i32 = y;
        let _e76: i32 = aa;
        if !((_e75 <= _e76)) {
            break;
        }
        {
            x = 0i;
            loop {
                let _e84: i32 = x;
                let _e85: i32 = aa;
                if !((_e84 <= _e85)) {
                    break;
                }
                {
                    let _e91: vec2<i32> = i0_1;
                    let _e92: i32 = x;
                    let _e93: i32 = y;
                    ti_1 = (_e91 + vec2<i32>(_e92, _e93));
                    let _e97: vec2<i32> = ti_1;
                    let _e102: vec2<i32> = ti_1;
                    let _e103: Tile = t_5;
                    if (any((_e97 < vec2(0i))) || any((_e102 >= _e103.rect.zw))) {
                        {
                            continue;
                        }
                    }
                    let _e109: vec2<f32> = hi;
                    let _e110: vec2<i32> = ti_1;
                    let _e116: vec2<f32> = lo;
                    let _e117: vec2<i32> = ti_1;
                    area = max((min(_e109, vec2<f32>((_e110 + vec2(1i)))) - max(_e116, vec2<f32>(_e117))), vec2(0f));
                    let _e125: vec2<f32> = area;
                    let _e127: vec2<f32> = area;
                    let _e130: f32 = weight_3;
                    let _e132: f32 = scale;
                    let _e133: f32 = scale;
                    w_1 = (((_e125.x * _e127.y) * _e130) / (_e132 * _e133));
                    let _e137: f32 = w_1;
                    if (_e137 <= 0f) {
                        {
                            continue;
                        }
                    }
                    let _e140: Tile = t_5;
                    let _e143: vec2<i32> = ti_1;
                    at_1 = (_e140.rect.xy + _e143);
                    let _e146: vec2<i32> = at_1;
                    let _e148: vec4<f32> = textureLoad(wgpu_atlasColour, _e146, 0i);
                    c_1 = _e148;
                    let _e150: vec4<f32> = c_1;
                    if (_e150.w <= 0f) {
                        {
                            continue;
                        }
                    }
                    let _e154: vec2<i32> = at_1;
                    let _e156: vec4<f32> = textureLoad(wgpu_atlasZ, _e154, 0i);
                    tz_1 = _e156.x;
                    let _e159: f32 = tz_1;
                    let _e160: f32 = wz_3;
                    let _e161: FigurePush = pc;
                    if (_e159 > (_e160 + _e161.atlas.z)) {
                        {
                            continue;
                        }
                    }
                    let _e166: f32 = (*zNear_1);
                    let _e167: f32 = tz_1;
                    (*zNear_1) = min(_e166, _e167);
                    let _e169: vec4<f32> = (*col_1);
                    let _e170: f32 = w_1;
                    let _e171: vec4<f32> = c_1;
                    let _e173: vec4<f32> = c_1;
                    let _e175: vec3<f32> = (_e171.xyz * _e173.w);
                    let _e176: vec4<f32> = c_1;
                    (*col_1) = (_e169 + (_e170 * vec4<f32>(_e175.x, _e175.y, _e175.z, _e176.w)));
                    let _e184: vec3<f32> = (*em_1);
                    let _e185: f32 = w_1;
                    let _e186: vec4<f32> = c_1;
                    let _e189: vec2<i32> = at_1;
                    let _e191: vec4<f32> = textureLoad(wgpu_atlasEmit, _e189, 0i);
                    (*em_1) = (_e184 + ((_e185 * _e186.w) * _e191.xyz));
                }
                continuing {
                    let _e88: i32 = x;
                    x = (_e88 + 1i);
                }
            }
        }
        continuing {
            let _e79: i32 = y;
            y = (_e79 + 1i);
        }
    }
    return;
}

fn main_1() {
    var px: vec2<i32>;
    var wz_4: f32;
    var f_1: Fig;
    var c_2: vec2<f32>;
    var col_2: vec4<f32> = vec4(0f);
    var em_2: vec3<f32> = vec3(0f);
    var zNear_2: f32 = 300000000000000000000000000000000000000f;
    var local_4: f32;
    var wb: f32;
    var a: Tile;
    var b: Tile;
    var fade: f32;

    let _e27: vec4<f32> = gl_FragCoord_1;
    px = vec2<i32>(_e27.xy);
    let _e31: vec2<i32> = px;
    let _e35: vec2<i32> = px;
    let _e40: vec2<i32> = px;
    let _e42: FigurePush = pc;
    let _e47: vec2<i32> = px;
    let _e49: FigurePush = pc;
    if ((((_e31.x < 0i) || (_e35.y < 0i)) || (_e40.x >= _e42.k.x)) || (_e47.y >= _e49.k.w)) {
        {
            discard;
        }
    }
    let _e54: vec2<i32> = px;
    let _e56: vec4<f32> = textureLoad(wgpu_walkDepth, _e54, 0i);
    wz_4 = _e56.x;
    let _e59: i32 = vFig_1;
    let _e61: Fig = global_2.figs[_e59];
    f_1 = _e61;
    let _e63: vec4<f32> = gl_FragCoord_1;
    let _e65: Fig = f_1;
    c_2 = (_e63.xy - _e65.anchor.xy);
    let _e78: Fig = f_1;
    if (_e78.tiles.y >= 0i) {
        let _e83: Fig = f_1;
        local_4 = clamp(_e83.anchor.z, 0f, 1f);
    } else {
        local_4 = 0f;
    }
    let _e91: f32 = local_4;
    wb = _e91;
    let _e93: Fig = f_1;
    let _e97: Tile = global_1.tiles[_e93.tiles.x];
    a = _e97;
    let _e99: Tile = a;
    let _e100: vec2<f32> = c_2;
    let _e101: Tile = a;
    let _e106: f32 = wz_4;
    let _e108: f32 = wb;
    tileOver(_e99, (_e100 - vec2<f32>(_e101.orig.xy)), _e106, (1f - _e108), (&col_2), (&em_2), (&zNear_2));
    let _e116: Fig = f_1;
    if (_e116.tiles.y >= 0i) {
        {
            let _e121: Fig = f_1;
            let _e125: Tile = global_1.tiles[_e121.tiles.y];
            b = _e125;
            let _e127: Tile = b;
            let _e128: vec2<f32> = c_2;
            let _e129: Tile = b;
            let _e134: f32 = wz_4;
            let _e135: f32 = wb;
            tileOver(_e127, (_e128 - vec2<f32>(_e129.orig.xy)), _e134, _e135, (&col_2), (&em_2), (&zNear_2));
        }
    }
    let _e142: vec4<f32> = col_2;
    if (_e142.w <= 0f) {
        {
            discard;
        }
    }
    let _e147: f32 = zNear_2;
    let _e150: FigurePush = pc;
    gl_FragDepth = clamp(((_e147 + 16f) / (_e150.atlas.w + 32f)), 0f, 1f);
    let _e159: Fig = f_1;
    fade = clamp(_e159.anchor.w, 0f, 1f);
    let _e166: vec4<f32> = col_2;
    let _e167: f32 = fade;
    col_2 = (_e166 * _e167);
    let _e169: vec3<f32> = em_2;
    let _e170: f32 = fade;
    em_2 = (_e169 * _e170);
    let _e172: vec4<f32> = col_2;
    let _e174: vec4<f32> = col_2;
    let _e176: vec4<f32> = col_2;
    let _e179: Fig = f_1;
    let _e186: vec3<f32> = mix(_e174.xyz, vec3(_e176.w), vec3(clamp(_e179.k.x, 0f, 1f)));
    col_2.x = _e186.x;
    col_2.y = _e186.y;
    col_2.z = _e186.z;
    let _e193: vec4<f32> = col_2;
    outColour = _e193;
    let _e194: vec4<f32> = col_2;
    figureWeatherSurface = _e194;
    let _e195: vec3<f32> = em_2;
    let _e196: vec4<f32> = col_2;
    outEmit = vec4<f32>(_e195.x, _e195.y, _e195.z, _e196.w);
    return;
}

@fragment 
fn main(@location(0) @interpolate(flat) vFig: i32, @builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    vFig_1 = vFig;
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e44: vec4<f32> = outColour;
    let _e46: vec4<f32> = outEmit;
    let _e48: vec4<f32> = figureWeatherSurface;
    let _e50: f32 = gl_FragDepth;
    return FragmentOutput(_e44, _e46, _e48, _e50);
}
