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

fn sbyte(v_2: u32, at: i32) -> i32 {
    var v_3: u32;
    var at_1: i32;

    v_3 = v_2;
    at_1 = at;
    let _e22: u32 = v_3;
    let _e24: i32 = at_1;
    return i32(extractBits(i32(_e22), u32(_e24), 8u));
}

fn main_1() {
    var t_2: Tile;
    var cell: vec4<u32>;
    var face: i32;
    var local_2: array<vec2<f32>, 6> = CORNER;
    var ab: vec2<f32>;
    var cls: i32;
    var head: bool;
    var c: vec3<f32>;
    var local_3: f32;
    var settle: f32;
    var local_4: f32;
    var breath: f32;
    var y0_: f32;
    var y1_: f32;
    var local_5: f32;
    var local_6: array<vec3<f32>, 6> = FACE_O;
    var local_7: array<vec3<f32>, 6> = FACE_U;
    var local_8: array<vec3<f32>, 6> = FACE_V;
    var unit: vec3<f32>;
    var local_9: vec3<f32>;
    var m: mat3x3<f32>;
    var row_6: i32;
    var centre: vec3<f32>;
    var q_1: vec3<f32>;
    var mid: vec3<f32>;
    var w: f32;
    var shift: f32;
    var fx: f32;
    var fz: f32;
    var map: vec2<f32>;

    let _e18: FigurePush = pc;
    let _e22: Tile = global_1.tiles[_e18.k.x];
    t_2 = _e22;
    let _e25: u32 = gl_InstanceIndex_1;
    let _e28: vec4<u32> = global.vox[i32(_e25)];
    cell = _e28;
    let _e31: u32 = gl_VertexIndex_1;
    face = (i32(_e31) / 6i);
    let _e36: u32 = gl_VertexIndex_1;
    let _e43: vec2<f32> = local_2[(i32(_e36) % 6i)];
    ab = _e43;
    let _e45: vec4<u32> = cell;
    cls = i32((_e45.y & 7u));
    let _e51: vec4<u32> = cell;
    head = ((_e51.y & 8u) != 0u);
    let _e58: vec4<u32> = cell;
    let _e61: i32 = sbyte(_e58.x, 0i);
    let _e63: vec4<u32> = cell;
    let _e66: i32 = sbyte(_e63.x, 8i);
    let _e68: vec4<u32> = cell;
    let _e71: i32 = sbyte(_e68.x, 16i);
    c = vec3<f32>(f32(_e61), f32(_e66), f32(_e71));
    let _e75: bool = head;
    if _e75 {
        let _e76: Tile = t_2;
        local_3 = _e76.az.w;
    } else {
        let _e79: Tile = t_2;
        local_3 = _e79.ax.w;
    }
    let _e83: f32 = local_3;
    settle = _e83;
    let _e85: bool = head;
    if _e85 {
        let _e86: Tile = t_2;
        local_4 = _e86.depth.z;
    } else {
        let _e89: Tile = t_2;
        local_4 = _e89.ay.w;
    }
    let _e93: f32 = local_4;
    breath = _e93;
    let _e95: vec3<f32> = c;
    y0_ = _e95.y;
    let _e98: vec3<f32> = c;
    y1_ = (_e98.y + 1f);
    let _e103: i32 = cls;
    let _e106: i32 = cls;
    if ((_e103 == 1i) || (_e106 == 2i)) {
        {
            let _e110: f32 = y0_;
            let _e111: f32 = settle;
            y0_ = (_e110 - _e111);
            let _e113: f32 = y1_;
            let _e114: f32 = settle;
            let _e115: i32 = cls;
            if (_e115 == 2i) {
                let _e118: f32 = breath;
                local_5 = _e118;
            } else {
                local_5 = 0f;
            }
            let _e121: f32 = local_5;
            y1_ = (_e113 - (_e114 + _e121));
        }
    } else {
        let _e124: i32 = cls;
        let _e127: i32 = cls;
        if ((_e124 == 3i) || (_e127 == 4i)) {
            {
                let _e131: f32 = y0_;
                let _e132: f32 = y1_;
                let _e133: vec3<f32> = c;
                let _e135: i32 = cls;
                let _e140: f32 = settle;
                y1_ = max(_e131, min(_e132, ((_e133.y + f32((_e135 - 2i))) - _e140)));
            }
        }
    }
    let _e144: i32 = face;
    let _e148: vec3<f32> = local_6[_e144];
    let _e149: vec2<f32> = ab;
    let _e151: i32 = face;
    let _e155: vec3<f32> = local_7[_e151];
    let _e158: vec2<f32> = ab;
    let _e160: i32 = face;
    let _e164: vec3<f32> = local_8[_e160];
    unit = ((_e148 + (_e149.x * _e155)) + (_e158.y * _e164));
    let _e168: vec3<f32> = c;
    let _e172: vec3<f32> = unit;
    let _e175: f32 = y0_;
    let _e176: f32 = y1_;
    let _e177: vec3<f32> = unit;
    let _e180: vec3<f32> = c;
    let _e184: vec3<f32> = unit;
    local_9 = vec3<f32>(((_e168.x - 0.5f) + _e172.x), mix(_e175, _e176, _e177.y), ((_e180.z - 0.5f) + _e184.z));
    let _e189: Tile = t_2;
    let _e191: vec3<f32> = _e189.ax.xyz;
    let _e192: Tile = t_2;
    let _e194: vec3<f32> = _e192.ay.xyz;
    let _e195: Tile = t_2;
    let _e197: vec3<f32> = _e195.az.xyz;
    m = mat3x3<f32>(vec3<f32>(_e191.x, _e191.y, _e191.z), vec3<f32>(_e194.x, _e194.y, _e194.z), vec3<f32>(_e197.x, _e197.y, _e197.z));
    let _e212: Tile = t_2;
    let _e213: vec4<u32> = cell;
    let _e215: i32 = boneRow(_e212, _e213.y);
    row_6 = _e215;
    let _e217: vec3<f32> = c;
    let _e220: f32 = y0_;
    let _e221: f32 = y1_;
    let _e224: vec3<f32> = c;
    centre = vec3<f32>(_e217.x, (0.5f * (_e220 + _e221)), _e224.z);
    let _e228: Tile = t_2;
    let _e231: mat3x3<f32> = m;
    let _e232: i32 = row_6;
    let _e233: vec3<f32> = local_9;
    let _e234: vec3<f32> = bonePose(_e232, _e233);
    q_1 = (_e228.pos.xyz + (_e231 * _e234));
    let _e238: Tile = t_2;
    let _e241: mat3x3<f32> = m;
    let _e242: i32 = row_6;
    let _e243: vec3<f32> = centre;
    let _e244: vec3<f32> = bonePose(_e242, _e243);
    mid = (_e238.pos.xyz + (_e241 * _e244));
    let _e248: FigurePush = pc;
    w = f32(_e248.k.y);
    let _e253: f32 = w;
    let _e254: vec3<f32> = mid;
    let _e256: vec3<f32> = mid;
    let _e258: FigurePush = pc;
    let _e263: f32 = w;
    shift = (_e253 * floor(((_e254.x - (_e256.y * _e258.atlas.x)) / _e263)));
    let _e268: vec3<f32> = q_1;
    let _e270: vec3<f32> = q_1;
    let _e272: FigurePush = pc;
    let _e277: f32 = shift;
    fx = ((_e268.x - (_e270.y * _e272.atlas.x)) - _e277);
    let _e280: vec3<f32> = q_1;
    let _e282: vec3<f32> = q_1;
    let _e284: FigurePush = pc;
    let _e289: FigurePush = pc;
    fz = ((_e280.z - (_e282.y * _e284.atlas.y)) - _e289.atlas.z);
    let _e294: f32 = fx;
    let _e295: f32 = w;
    let _e297: f32 = fz;
    let _e298: FigurePush = pc;
    let _e302: FigurePush = pc;
    map = vec2<f32>((_e294 / _e295), ((_e297 * _e298.atlas.w) / f32(_e302.k.w)));
    let _e310: vec2<f32> = map;
    let _e315: vec2<f32> = ((_e310 * 2f) - vec2(1f));
    let _e316: vec3<f32> = q_1;
    let _e318: FigurePush = pc;
    gl_Position = vec4<f32>(_e315.x, _e315.y, clamp((_e316.y / f32(_e318.k.z)), 0f, 1f), 1f);
    let _e331: vec4<f32> = gl_Position;
    gl_Position.y = -(_e331.y);
    return;
}

@vertex 
fn main(@builtin(instance_index) gl_InstanceIndex: u32, @builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    gl_InstanceIndex_1 = gl_InstanceIndex;
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e28: vec4<f32> = gl_Position;
    return VertexOutput(_e28);
}
