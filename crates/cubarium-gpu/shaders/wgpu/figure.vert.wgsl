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

struct VertexOutput {
    @location(0) vWorld: vec3<f32>,
    @location(1) vNormal: vec3<f32>,
    @location(2) vFace: vec2<f32>,
    @location(3) @interpolate(flat) vInfo: vec4<i32>,
    @location(4) @interpolate(flat) vFaceSize: vec2<i32>,
    @location(5) vRimWorld: vec3<f32>,
    @location(6) vRimFace: vec2<f32>,
    @location(7) @interpolate(flat) vCapSize: vec2<i32>,
    @builtin(position) gl_Position: vec4<f32>,
}

const FACE_O: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, 0f), vec3<f32>(1f, 0f, 1f));
const FACE_U: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(-1f, 0f, 0f));
const FACE_V: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f));
const FACE_N: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(-1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, -1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f));
const CORNER: array<vec2<f32>, 6> = array<vec2<f32>, 6>(vec2<f32>(0f, 0f), vec2<f32>(1f, 0f), vec2<f32>(1f, 1f), vec2<f32>(0f, 0f), vec2<f32>(1f, 1f), vec2<f32>(0f, 1f));

var<immediate> pc: FigurePush;
@group(1) @binding(0) 
var<storage> global: FigureVoxels;
@group(1) @binding(1) 
var<storage> global_1: FigureTiles;
@group(1) @binding(2) 
var<storage> global_2: FigureFigs;
@group(1) @binding(4) 
var<storage> global_3: FigureBones;
var<private> vWorld: vec3<f32>;
var<private> vNormal: vec3<f32>;
var<private> vFace: vec2<f32>;
var<private> vInfo: vec4<i32>;
var<private> vFaceSize: vec2<i32>;
var<private> vRimWorld: vec3<f32>;
var<private> vRimFace: vec2<f32>;
var<private> vCapSize: vec2<i32>;
var<private> gl_InstanceIndex_1: u32;
var<private> gl_VertexIndex_1: u32;
var<private> gl_Position: vec4<f32>;

fn boneRow(t: Tile, bits: u32) -> i32 {
    var t_1: Tile;
    var bits_1: u32;
    var local: i32;

    t_1 = t;
    bits_1 = bits;
    let _e17: Tile = t_1;
    if (_e17.rig.x < 0i) {
        local = -1i;
    } else {
        let _e25: Tile = t_1;
        let _e28: u32 = bits_1;
        local = (3i * (_e25.rig.x + i32(((_e28 >> 14u) & 63u))));
    }
    let _e38: i32 = local;
    return _e38;
}

fn boneMove(row: i32, p: vec3<f32>) -> vec3<f32> {
    var row_1: i32;
    var p_1: vec3<f32>;
    var q: vec4<f32>;

    row_1 = row;
    p_1 = p;
    let _e17: vec3<f32> = p_1;
    q = vec4<f32>(_e17.x, _e17.y, _e17.z, 1f);
    let _e24: i32 = row_1;
    let _e26: vec4<f32> = global_3.bones[_e24];
    let _e27: vec4<f32> = q;
    let _e29: i32 = row_1;
    let _e33: vec4<f32> = global_3.bones[(_e29 + 1i)];
    let _e34: vec4<f32> = q;
    let _e36: i32 = row_1;
    let _e40: vec4<f32> = global_3.bones[(_e36 + 2i)];
    let _e41: vec4<f32> = q;
    return vec3<f32>(dot(_e26, _e27), dot(_e33, _e34), dot(_e40, _e41));
}

fn boneTurn(row_2: i32, v: vec3<f32>) -> vec3<f32> {
    var row_3: i32;
    var v_1: vec3<f32>;

    row_3 = row_2;
    v_1 = v;
    let _e17: i32 = row_3;
    let _e19: vec4<f32> = global_3.bones[_e17];
    let _e21: vec3<f32> = v_1;
    let _e23: i32 = row_3;
    let _e27: vec4<f32> = global_3.bones[(_e23 + 1i)];
    let _e29: vec3<f32> = v_1;
    let _e31: i32 = row_3;
    let _e35: vec4<f32> = global_3.bones[(_e31 + 2i)];
    let _e37: vec3<f32> = v_1;
    return vec3<f32>(dot(_e19.xyz, _e21), dot(_e27.xyz, _e29), dot(_e35.xyz, _e37));
}

fn bonePose(row_4: i32, p_2: vec3<f32>) -> vec3<f32> {
    var row_5: i32;
    var p_3: vec3<f32>;
    var local_1: vec3<f32>;

    row_5 = row_4;
    p_3 = p_2;
    let _e17: i32 = row_5;
    if (_e17 < 0i) {
        let _e20: vec3<f32> = p_3;
        local_1 = _e20;
    } else {
        let _e21: i32 = row_5;
        let _e22: vec3<f32> = p_3;
        let _e23: vec3<f32> = boneMove(_e21, _e22);
        local_1 = _e23;
    }
    let _e25: vec3<f32> = local_1;
    return _e25;
}

fn faceSize(t_2: Tile, face: i32) -> vec2<i32> {
    var t_3: Tile;
    var face_1: i32;

    t_3 = t_2;
    face_1 = face;
    let _e29: i32 = face_1;
    if (_e29 < 2i) {
        {
            let _e32: Tile = t_3;
            return _e32.faceSize.zy;
        }
    }
    let _e35: i32 = face_1;
    if (_e35 < 4i) {
        {
            let _e38: Tile = t_3;
            return _e38.faceSize.xz;
        }
    }
    let _e41: Tile = t_3;
    return _e41.faceSize.xy;
}

fn sbyte(v_2: u32, at: i32) -> i32 {
    var v_3: u32;
    var at_1: i32;

    v_3 = v_2;
    at_1 = at;
    let _e30: u32 = v_3;
    let _e32: i32 = at_1;
    return i32(extractBits(i32(_e30), u32(_e32), 8u));
}

fn main_1() {
    var t_4: Tile;
    var cell: vec4<u32>;
    var face_2: i32;
    var local_2: array<vec2<f32>, 6> = CORNER;
    var ab: vec2<f32>;
    var cls: i32;
    var head: bool;
    var open: u32;
    var glyph: i32;
    var row_6: i32;
    var local_3: array<vec3<f32>, 6> = FACE_N;
    var local_4: array<vec3<f32>, 6> = FACE_N;
    var local_5: vec3<f32>;
    var nLocal: vec3<f32>;
    var n: vec3<f32>;
    var seen: bool;
    var c: vec3<f32>;
    var local_6: f32;
    var settle: f32;
    var local_7: f32;
    var breath: f32;
    var y0_: f32;
    var y1_: f32;
    var local_8: f32;
    var d: f32;
    var local_9: array<vec3<f32>, 6> = FACE_O;
    var local_10: array<vec3<f32>, 6> = FACE_U;
    var local_11: array<vec3<f32>, 6> = FACE_V;
    var unit: vec3<f32>;
    var local_12: vec3<f32>;
    var posed: vec3<f32>;
    var rel: vec3<f32>;
    var q_1: vec3<f32>;
    var screen: vec2<f32>;
    var local_13: vec2<f32>;
    var atlas: vec2<f32>;
    var depth: f32;
    var inward: f32;
    var local_14: array<vec3<f32>, 6> = FACE_N;
    var rim: vec3<f32>;
    var local_15: array<vec3<f32>, 6> = FACE_N;
    var rimLocal: vec3<f32>;

    let _e26: FigurePush = pc;
    let _e30: Tile = global_1.tiles[_e26.k.x];
    t_4 = _e30;
    let _e33: u32 = gl_InstanceIndex_1;
    let _e36: vec4<u32> = global.vox[i32(_e33)];
    cell = _e36;
    let _e39: u32 = gl_VertexIndex_1;
    face_2 = (i32(_e39) / 6i);
    let _e44: u32 = gl_VertexIndex_1;
    let _e51: vec2<f32> = local_2[(i32(_e44) % 6i)];
    ab = _e51;
    let _e53: vec4<u32> = cell;
    cls = i32((_e53.y & 7u));
    let _e59: vec4<u32> = cell;
    head = ((_e59.y & 8u) != 0u);
    let _e66: vec4<u32> = cell;
    open = ((_e66.y >> 4u) & 63u);
    let _e74: vec4<u32> = cell;
    glyph = i32(((_e74.y >> 10u) & 15u));
    let _e83: Tile = t_4;
    let _e84: vec4<u32> = cell;
    let _e86: i32 = boneRow(_e83, _e84.y);
    row_6 = _e86;
    let _e88: i32 = row_6;
    if (_e88 >= 0i) {
        let _e91: i32 = row_6;
        let _e92: i32 = face_2;
        let _e96: vec3<f32> = local_3[_e92];
        let _e97: vec3<f32> = boneTurn(_e91, _e96);
        local_5 = _e97;
    } else {
        let _e98: i32 = face_2;
        let _e102: vec3<f32> = local_4[_e98];
        local_5 = _e102;
    }
    let _e104: vec3<f32> = local_5;
    nLocal = _e104;
    let _e106: Tile = t_4;
    let _e108: vec3<f32> = _e106.ax.xyz;
    let _e109: Tile = t_4;
    let _e111: vec3<f32> = _e109.ay.xyz;
    let _e112: Tile = t_4;
    let _e114: vec3<f32> = _e112.az.xyz;
    let _e128: vec3<f32> = nLocal;
    n = (mat3x3<f32>(vec3<f32>(_e108.x, _e108.y, _e108.z), vec3<f32>(_e111.x, _e111.y, _e111.z), vec3<f32>(_e114.x, _e114.y, _e114.z)) * _e128);
    let _e131: u32 = open;
    let _e133: i32 = face_2;
    let _e139: vec3<f32> = n;
    let _e141: FigurePush = pc;
    let _e146: vec3<f32> = n;
    let _e148: FigurePush = pc;
    seen = (((_e131 & (1u << u32(_e133))) != 0u) && (((_e139.z * f32(_e141.k.y)) - (_e146.y * f32(_e148.k.z))) < 0.0001f));
    let _e158: bool = seen;
    if !(_e158) {
        {
            gl_Position = vec4<f32>(2f, 2f, 2f, 1f);
            let _e167: vec4<f32> = gl_Position;
            gl_Position.y = -(_e167.y);
            vWorld = vec3(0f);
            vNormal = vec3<f32>(0f, 1f, 0f);
            vFace = vec2(0f);
            vInfo = vec4(0i);
            let _e181: vec2<i32> = vec2(1i);
            vCapSize = _e181;
            vFaceSize = _e181;
            vRimWorld = vec3(0f);
            vRimFace = vec2(0f);
            return;
        }
    }
    let _e186: vec4<u32> = cell;
    let _e189: i32 = sbyte(_e186.x, 0i);
    let _e191: vec4<u32> = cell;
    let _e194: i32 = sbyte(_e191.x, 8i);
    let _e196: vec4<u32> = cell;
    let _e199: i32 = sbyte(_e196.x, 16i);
    c = vec3<f32>(f32(_e189), f32(_e194), f32(_e199));
    let _e203: bool = head;
    if _e203 {
        let _e204: Tile = t_4;
        local_6 = _e204.az.w;
    } else {
        let _e207: Tile = t_4;
        local_6 = _e207.ax.w;
    }
    let _e211: f32 = local_6;
    settle = _e211;
    let _e213: bool = head;
    if _e213 {
        let _e214: Tile = t_4;
        local_7 = _e214.depth.z;
    } else {
        let _e217: Tile = t_4;
        local_7 = _e217.ay.w;
    }
    let _e221: f32 = local_7;
    breath = _e221;
    let _e223: vec3<f32> = c;
    y0_ = _e223.y;
    let _e226: vec3<f32> = c;
    y1_ = (_e226.y + 1f);
    let _e231: i32 = cls;
    let _e234: i32 = cls;
    if ((_e231 == 1i) || (_e234 == 2i)) {
        {
            let _e238: f32 = y0_;
            let _e239: f32 = settle;
            y0_ = (_e238 - _e239);
            let _e241: f32 = y1_;
            let _e242: f32 = settle;
            let _e243: i32 = cls;
            if (_e243 == 2i) {
                let _e246: f32 = breath;
                local_8 = _e246;
            } else {
                local_8 = 0f;
            }
            let _e249: f32 = local_8;
            y1_ = (_e241 - (_e242 + _e249));
        }
    } else {
        let _e252: i32 = cls;
        let _e255: i32 = cls;
        if ((_e252 == 3i) || (_e255 == 4i)) {
            {
                let _e259: i32 = cls;
                d = f32((_e259 - 2i));
                let _e264: f32 = y0_;
                let _e265: f32 = y1_;
                let _e266: vec3<f32> = c;
                let _e268: f32 = d;
                let _e270: f32 = settle;
                y1_ = max(_e264, min(_e265, ((_e266.y + _e268) - _e270)));
            }
        }
    }
    let _e274: i32 = face_2;
    let _e278: vec3<f32> = local_9[_e274];
    let _e279: vec2<f32> = ab;
    let _e281: i32 = face_2;
    let _e285: vec3<f32> = local_10[_e281];
    let _e288: vec2<f32> = ab;
    let _e290: i32 = face_2;
    let _e294: vec3<f32> = local_11[_e290];
    unit = ((_e278 + (_e279.x * _e285)) + (_e288.y * _e294));
    let _e298: vec3<f32> = c;
    let _e302: vec3<f32> = unit;
    let _e305: f32 = y0_;
    let _e306: f32 = y1_;
    let _e307: vec3<f32> = unit;
    let _e310: vec3<f32> = c;
    let _e314: vec3<f32> = unit;
    local_12 = vec3<f32>(((_e298.x - 0.5f) + _e302.x), mix(_e305, _e306, _e307.y), ((_e310.z - 0.5f) + _e314.z));
    let _e319: i32 = row_6;
    let _e320: vec3<f32> = local_12;
    let _e321: vec3<f32> = bonePose(_e319, _e320);
    posed = _e321;
    let _e323: Tile = t_4;
    let _e326: vec3<f32> = posed;
    let _e329: Tile = t_4;
    let _e332: vec3<f32> = posed;
    let _e336: Tile = t_4;
    let _e339: vec3<f32> = posed;
    rel = (((_e323.ax.xyz * _e326.x) + (_e329.ay.xyz * _e332.y)) + (_e336.az.xyz * _e339.z));
    let _e344: Tile = t_4;
    let _e347: vec3<f32> = rel;
    q_1 = (_e344.pos.xyz + _e347);
    let _e350: vec3<f32> = rel;
    let _e352: FigurePush = pc;
    let _e357: vec3<f32> = rel;
    let _e359: FigurePush = pc;
    let _e364: vec3<f32> = rel;
    let _e366: FigurePush = pc;
    screen = vec2<f32>((_e350.x * f32(_e352.k.y)), -(((_e357.y * f32(_e359.k.y)) + (_e364.z * f32(_e366.k.z)))));
    let _e375: vec2<f32> = screen;
    let _e376: FigurePush = pc;
    let _e381: FigurePush = pc;
    screen = (_e375 - (vec2<f32>(f32((_e376.k.y & 1i)), f32((_e381.k.z & 1i))) * 0.5f));
    let _e392: Tile = t_4;
    if (_e392.faceSize.w == 1i) {
        let _e397: Tile = t_4;
        let _e401: vec2<f32> = screen;
        let _e403: Tile = t_4;
        local_13 = ((vec2<f32>(_e397.rect.xy) + _e401) - vec2<f32>(_e403.orig.xy));
    } else {
        let _e408: Tile = t_4;
        let _e412: vec2<f32> = screen;
        let _e413: Tile = t_4;
        let _e418: Tile = t_4;
        local_13 = (vec2<f32>(_e408.rect.xy) + ((_e412 - vec2<f32>(_e413.orig.xy)) * f32(_e418.faceSize.w)));
    }
    let _e425: vec2<f32> = local_13;
    atlas = _e425;
    let _e428: vec3<f32> = rel;
    let _e430: Tile = t_4;
    depth = clamp((0.5f + (_e428.z * _e430.depth.y)), 0f, 1f);
    let _e439: vec2<f32> = atlas;
    let _e440: FigurePush = pc;
    let _e448: vec2<f32> = (((_e439 / _e440.atlas.xy) * 2f) - vec2(1f));
    let _e449: f32 = depth;
    gl_Position = vec4<f32>(_e448.x, _e448.y, _e449, 1f);
    let _e455: vec4<f32> = gl_Position;
    gl_Position.y = -(_e455.y);
    let _e458: vec3<f32> = q_1;
    vWorld = _e458;
    let _e459: Tile = t_4;
    let _e460: i32 = face_2;
    let _e461: vec2<i32> = faceSize(_e459, _e460);
    vFaceSize = _e461;
    let _e462: Tile = t_4;
    let _e464: vec2<i32> = faceSize(_e462, 3i);
    vCapSize = _e464;
    let _e466: vec2<i32> = vCapSize;
    let _e468: vec2<i32> = vCapSize;
    inward = (0.5f / f32(min(_e466.x, _e468.y)));
    let _e474: vec3<f32> = unit;
    let _e475: i32 = face_2;
    let _e479: vec3<f32> = local_14[_e475];
    let _e480: f32 = inward;
    rim = (_e474 - (_e479 * _e480));
    let _e484: i32 = row_6;
    let _e485: vec3<f32> = local_12;
    let _e487: f32 = y1_;
    let _e488: vec3<f32> = local_12;
    let _e491: i32 = face_2;
    let _e495: vec3<f32> = local_15[_e491];
    let _e496: f32 = inward;
    let _e499: vec3<f32> = bonePose(_e484, (vec3<f32>(_e485.x, _e487, _e488.z) - (_e495 * _e496)));
    rimLocal = _e499;
    let _e501: Tile = t_4;
    let _e504: Tile = t_4;
    let _e506: vec3<f32> = _e504.ax.xyz;
    let _e507: Tile = t_4;
    let _e509: vec3<f32> = _e507.ay.xyz;
    let _e510: Tile = t_4;
    let _e512: vec3<f32> = _e510.az.xyz;
    let _e526: vec3<f32> = rimLocal;
    vRimWorld = (_e501.pos.xyz + (mat3x3<f32>(vec3<f32>(_e506.x, _e506.y, _e506.z), vec3<f32>(_e509.x, _e509.y, _e509.z), vec3<f32>(_e512.x, _e512.y, _e512.z)) * _e526));
    let _e529: vec3<f32> = rim;
    let _e532: vec3<f32> = rim;
    vRimFace = vec2<f32>(_e529.x, (1f - _e532.z));
    let _e536: vec3<f32> = n;
    vNormal = _e536;
    let _e537: vec2<f32> = ab;
    let _e540: vec2<f32> = ab;
    vFace = vec2<f32>(_e537.x, (1f - _e540.y));
    let _e544: Tile = t_4;
    let _e547: vec4<u32> = cell;
    let _e554: i32 = glyph;
    let _e555: vec4<u32> = cell;
    let _e563: i32 = face_2;
    let _e565: i32 = face_2;
    let _e570: u32 = cell[(2i + (_e565 / 4i))];
    let _e572: i32 = face_2;
    let _e585: vec4<u32> = cell;
    vInfo = vec4<i32>((_e544.orig.z + i32((_e547.x >> 24u))), _e554, i32(((_e555.y >> 20u) & 1u)), ((_e563 | i32((((_e570 >> u32((8i * (_e572 % 4i)))) & 255u) << 3u))) | i32(((_e585.z >> 24u) << 11u))));
    return;
}

@vertex 
fn main(@builtin(instance_index) gl_InstanceIndex: u32, @builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    gl_InstanceIndex_1 = gl_InstanceIndex;
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e44: vec3<f32> = vWorld;
    let _e46: vec3<f32> = vNormal;
    let _e48: vec2<f32> = vFace;
    let _e50: vec4<i32> = vInfo;
    let _e52: vec2<i32> = vFaceSize;
    let _e54: vec3<f32> = vRimWorld;
    let _e56: vec2<f32> = vRimFace;
    let _e58: vec2<i32> = vCapSize;
    let _e60: vec4<f32> = gl_Position;
    return VertexOutput(_e44, _e46, _e48, _e50, _e52, _e54, _e56, _e58, _e60);
}
