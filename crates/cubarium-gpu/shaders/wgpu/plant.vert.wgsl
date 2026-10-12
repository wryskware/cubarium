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

struct VertexOutput {
    @location(0) @interpolate(flat) vId: u32,
    @location(1) vUV: vec2<f32>,
    @builtin(position) gl_Position: vec4<f32>,
}

const CORNER: array<vec2<f32>, 6> = array<vec2<f32>, 6>(vec2<f32>(0f, 0f), vec2<f32>(1f, 0f), vec2<f32>(1f, 1f), vec2<f32>(0f, 0f), vec2<f32>(1f, 1f), vec2<f32>(0f, 1f));

var<immediate> pc: PlantPush;
@group(1) @binding(0) 
var<storage> global: PlantCells;
@group(1) @binding(1) 
var<storage> global_1: PlantTips;
var<private> vId: u32;
var<private> vUV: vec2<f32>;
var<private> gl_Position: vec4<f32>;
var<private> gl_InstanceIndex_1: u32;
var<private> gl_VertexIndex_1: u32;

fn plantCellAt(c: vec4<u32>) -> vec3<i32> {
    var c_1: vec4<u32>;

    c_1 = c;
    let _e15: vec4<u32> = c_1;
    let _e20: vec4<u32> = c_1;
    let _e28: vec4<u32> = c_1;
    return vec3<i32>(i32((_e15.x & 4095u)), i32(((_e20.x >> 12u) & 1023u)), i32((_e28.x >> 22u)));
}

fn plantOf(c_2: vec4<u32>) -> u32 {
    var c_3: vec4<u32>;

    c_3 = c_2;
    let _e15: vec4<u32> = c_3;
    return (_e15.y & 524287u);
}

fn plantDying(c_4: vec4<u32>) -> bool {
    var c_5: vec4<u32>;

    c_5 = c_4;
    let _e15: vec4<u32> = c_5;
    return ((_e15.y & 524288u) != 0u);
}

fn plantWeight(c_6: vec4<u32>) -> f32 {
    var c_7: vec4<u32>;

    c_7 = c_6;
    let _e15: vec4<u32> = c_7;
    return (f32((_e15.y >> 20u)) / 4095f);
}

fn plantCarried(c_8: vec4<u32>) -> vec4<u32> {
    var c_9: vec4<u32>;

    c_9 = c_8;
    let _e15: vec4<u32> = c_9;
    let _e19: vec4<u32> = c_9;
    let _e26: vec4<u32> = c_9;
    let _e33: vec4<u32> = c_9;
    return vec4<u32>((_e15.w & 255u), ((_e19.w >> 8u) & 255u), ((_e26.w >> 16u) & 255u), (_e33.w >> 24u));
}

fn plantScale(c_10: vec4<u32>) -> f32 {
    var c_11: vec4<u32>;
    var since: f32;
    var local: f32;
    var local_1: f32;
    var r: f32;
    var s: f32;
    var local_2: f32;

    c_11 = c_10;
    let _e15: PlantPush = pc;
    let _e18: vec4<u32> = c_11;
    since = (_e15.t.x - bitcast<f32>(_e18.z));
    let _e23: PlantPush = pc;
    if (_e23.t.y > 0f) {
        let _e28: f32 = since;
        let _e29: PlantPush = pc;
        local_1 = clamp((_e28 / _e29.t.y), 0f, 1f);
    } else {
        let _e36: f32 = since;
        if (_e36 >= 0f) {
            local = 1f;
        } else {
            local = 0f;
        }
        let _e42: f32 = local;
        local_1 = _e42;
    }
    let _e44: f32 = local_1;
    r = _e44;
    let _e46: f32 = r;
    let _e47: f32 = r;
    let _e51: f32 = r;
    s = ((_e46 * _e47) * (3f - (2f * _e51)));
    let _e56: vec4<u32> = c_11;
    let _e57: bool = plantDying(_e56);
    if _e57 {
        let _e59: f32 = s;
        local_2 = (1f - _e59);
    } else {
        let _e61: f32 = s;
        local_2 = _e61;
    }
    let _e63: f32 = local_2;
    return _e63;
}

fn plantBox(c_12: vec4<u32>, ctr: ptr<function, vec3<f32>>, h: ptr<function, f32>) {
    var c_13: vec4<u32>;

    c_13 = c_12;
    let _e18: vec4<u32> = c_13;
    let _e19: f32 = plantScale(_e18);
    (*h) = (0.5f * _e19);
    let _e21: vec4<u32> = c_13;
    let _e22: vec3<i32> = plantCellAt(_e21);
    (*ctr) = (vec3<f32>(_e22) + vec3(0.5f));
    let _e28: vec3<f32> = (*ctr);
    let _e30: vec4<u32> = c_13;
    let _e31: u32 = plantOf(_e30);
    let _e33: f32 = global_1.tips[_e31];
    let _e34: vec4<u32> = c_13;
    let _e35: f32 = plantWeight(_e34);
    (*ctr).x = (_e28.x + (_e33 * _e35));
    return;
}

fn plantFaceZ(c_14: vec4<u32>, cap: bool, py: f32) -> f32 {
    var c_15: vec4<u32>;
    var cap_1: bool;
    var py_1: f32;
    var ctr_1: vec3<f32>;
    var h_1: f32;
    var zf: f32;
    var R: f32;
    var bottom: f32;

    c_15 = c_14;
    cap_1 = cap;
    py_1 = py;
    let _e21: vec4<u32> = c_15;
    plantBox(_e21, (&ctr_1), (&h_1));
    let _e26: vec3<f32> = ctr_1;
    let _e28: f32 = h_1;
    zf = (_e26.z - _e28);
    let _e31: bool = cap_1;
    if !(_e31) {
        {
            let _e33: f32 = zf;
            return _e33;
        }
    }
    let _e34: PlantPush = pc;
    R = f32(_e34.geom.y);
    let _e39: PlantPush = pc;
    let _e43: vec3<f32> = ctr_1;
    let _e45: f32 = h_1;
    let _e47: PlantPush = pc;
    let _e53: f32 = zf;
    let _e54: f32 = R;
    bottom = ((f32(_e39.geom.z) - ((_e43.y + _e45) * f32(_e47.geom.x))) - (_e53 * _e54));
    let _e58: f32 = zf;
    let _e59: f32 = bottom;
    let _e60: f32 = py_1;
    let _e62: f32 = R;
    let _e66: f32 = h_1;
    return (_e58 + clamp(((_e59 - _e60) / _e62), 0f, (2f * _e66)));
}

fn plantDepth(z: f32) -> f32 {
    var z_1: f32;

    z_1 = z;
    let _e15: f32 = z_1;
    let _e18: PlantPush = pc;
    return clamp(((_e15 + 16f) / (f32(_e18.k.w) + 32f)), 0f, 1f);
}

fn plantZ(depth: f32) -> f32 {
    var depth_1: f32;

    depth_1 = depth;
    let _e15: f32 = depth_1;
    let _e16: PlantPush = pc;
    return ((_e15 * (f32(_e16.k.w) + 32f)) - 16f);
}

fn cull() {
    gl_Position = vec4<f32>(2f, 2f, 2f, 1f);
    let _e23: vec4<f32> = gl_Position;
    gl_Position.y = -(_e23.y);
    vId = 0u;
    vUV = vec2(0f);
    return;
}

fn main_1() {
    var n: i32;
    var inst: i32;
    var copy: bool;
    var c_16: vec4<u32>;
    var k: f32;
    var face: i32;
    var local_3: array<vec2<f32>, 6> = CORNER;
    var ab: vec2<f32>;
    var S: f32;
    var R_1: f32;
    var B: f32;
    var ctr_2: vec3<f32>;
    var h_2: f32;
    var zf_1: f32;
    var top: f32;
    var bottom_1: f32;
    var d0_: f32;
    var d1_: f32;
    var yt: f32;
    var width: f32;
    var span: vec2<f32> = vec2<f32>(0f, 1f);
    var rw: f32;
    var rel: f32;
    var left: f32;
    var px: vec2<f32>;
    var aa: f32;

    let _e17: PlantPush = pc;
    n = _e17.k.x;
    let _e22: u32 = gl_InstanceIndex_1;
    inst = i32(_e22);
    let _e25: i32 = inst;
    let _e26: i32 = n;
    copy = (_e25 >= _e26);
    let _e29: bool = copy;
    if _e29 {
        {
            let _e30: i32 = inst;
            let _e31: i32 = n;
            inst = (_e30 - _e31);
        }
    }
    let _e33: i32 = inst;
    let _e35: vec4<u32> = global.cells[_e33];
    c_16 = _e35;
    let _e37: vec4<u32> = c_16;
    let _e38: f32 = plantScale(_e37);
    k = _e38;
    let _e40: f32 = k;
    if (_e40 <= 0f) {
        {
            cull();
            return;
        }
    }
    let _e44: u32 = gl_VertexIndex_1;
    face = (i32(_e44) / 6i);
    let _e49: u32 = gl_VertexIndex_1;
    let _e56: vec2<f32> = local_3[(i32(_e49) % 6i)];
    ab = _e56;
    let _e58: PlantPush = pc;
    S = f32(_e58.geom.x);
    let _e63: PlantPush = pc;
    R_1 = f32(_e63.geom.y);
    let _e68: PlantPush = pc;
    B = f32(_e68.geom.z);
    let _e75: vec4<u32> = c_16;
    plantBox(_e75, (&ctr_2), (&h_2));
    let _e80: vec3<f32> = ctr_2;
    let _e82: f32 = h_2;
    zf_1 = (_e80.z - _e82);
    let _e89: i32 = face;
    if (_e89 == 0i) {
        {
            let _e92: f32 = B;
            let _e93: vec3<f32> = ctr_2;
            let _e95: f32 = h_2;
            let _e97: f32 = S;
            let _e100: f32 = zf_1;
            let _e101: f32 = R_1;
            top = ((_e92 - ((_e93.y + _e95) * _e97)) - (_e100 * _e101));
            let _e104: f32 = B;
            let _e105: vec3<f32> = ctr_2;
            let _e107: f32 = h_2;
            let _e109: f32 = S;
            let _e112: f32 = zf_1;
            let _e113: f32 = R_1;
            bottom_1 = ((_e104 - ((_e105.y - _e107) * _e109)) - (_e112 * _e113));
            let _e116: f32 = zf_1;
            d0_ = _e116;
            let _e117: f32 = zf_1;
            d1_ = _e117;
        }
    } else {
        {
            let _e118: vec3<f32> = ctr_2;
            let _e120: f32 = h_2;
            yt = (_e118.y + _e120);
            let _e123: f32 = B;
            let _e124: f32 = yt;
            let _e125: f32 = S;
            let _e128: f32 = zf_1;
            let _e130: f32 = h_2;
            let _e133: f32 = R_1;
            top = ((_e123 - (_e124 * _e125)) - ((_e128 + (2f * _e130)) * _e133));
            let _e136: f32 = B;
            let _e137: f32 = yt;
            let _e138: f32 = S;
            let _e141: f32 = zf_1;
            let _e142: f32 = R_1;
            bottom_1 = ((_e136 - (_e137 * _e138)) - (_e141 * _e142));
            let _e145: f32 = zf_1;
            let _e147: f32 = h_2;
            d0_ = (_e145 + (2f * _e147));
            let _e150: f32 = zf_1;
            d1_ = _e150;
        }
    }
    let _e152: f32 = h_2;
    let _e154: f32 = S;
    width = ((2f * _e152) * _e154);
    let _e161: i32 = face;
    let _e164: PlantPush = pc;
    let _e170: vec4<u32> = c_16;
    let _e171: bool = plantDying(_e170);
    let _e172: vec4<u32> = c_16;
    if (((_e161 == 0i) && (_e164.t.w > 0.5f)) && (_e171 || ((_e172.w & 3221225472u) == 3221225472u))) {
        {
            let _e181: PlantPush = pc;
            let _e186: PlantPush = pc;
            span = vec2<f32>((0.5f - _e181.t.w), (0.5f + _e186.t.w));
        }
    }
    let _e191: PlantPush = pc;
    let _e195: f32 = S;
    rw = (f32(_e191.geom.w) * _e195);
    let _e198: vec3<f32> = ctr_2;
    let _e200: f32 = h_2;
    let _e202: f32 = S;
    let _e204: PlantPush = pc;
    rel = (((_e198.x - _e200) * _e202) - f32(_e204.k.z));
    let _e210: f32 = rel;
    let _e211: f32 = rw;
    let _e212: f32 = rel;
    let _e213: f32 = rw;
    left = (_e210 - (_e211 * floor((_e212 / _e213))));
    let _e219: bool = copy;
    if _e219 {
        {
            let _e220: f32 = left;
            let _e221: vec2<f32> = span;
            let _e223: f32 = width;
            let _e226: f32 = rw;
            if ((_e220 + (_e221.y * _e223)) > _e226) {
                {
                    let _e228: f32 = left;
                    let _e229: f32 = rw;
                    left = (_e228 - _e229);
                }
            } else {
                let _e231: f32 = left;
                let _e232: vec2<f32> = span;
                let _e234: f32 = width;
                if ((_e231 + (_e232.x * _e234)) < 0f) {
                    {
                        let _e239: f32 = left;
                        let _e240: f32 = rw;
                        left = (_e239 + _e240);
                    }
                } else {
                    {
                        cull();
                        return;
                    }
                }
            }
        }
    }
    let _e243: vec2<f32> = span;
    let _e245: vec2<f32> = span;
    let _e247: vec2<f32> = ab;
    ab.x = mix(_e243.x, _e245.y, _e247.x);
    let _e250: f32 = left;
    let _e251: vec2<f32> = ab;
    let _e253: f32 = width;
    let _e256: f32 = top;
    let _e257: f32 = bottom_1;
    let _e258: vec2<f32> = ab;
    px = vec2<f32>((_e250 + (_e251.x * _e253)), mix(_e256, _e257, _e258.y));
    let _e263: PlantPush = pc;
    aa = f32(_e263.k.y);
    let _e268: vec2<f32> = px;
    let _e269: f32 = aa;
    let _e271: PlantPush = pc;
    let _e279: vec2<f32> = ((((_e268 * _e269) / _e271.layer.xy) * 2f) - vec2(1f));
    let _e280: f32 = d0_;
    let _e281: f32 = d1_;
    let _e282: vec2<f32> = ab;
    let _e285: f32 = plantDepth(mix(_e280, _e281, _e282.y));
    gl_Position = vec4<f32>(_e279.x, _e279.y, _e285, 1f);
    let _e291: vec4<f32> = gl_Position;
    gl_Position.y = -(_e291.y);
    let _e294: i32 = inst;
    let _e299: i32 = face;
    vId = ((u32(_e294) << 1u) | u32(_e299));
    let _e302: vec2<f32> = ab;
    vUV = _e302;
    return;
}

@vertex 
fn main(@builtin(instance_index) gl_InstanceIndex: u32, @builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    gl_InstanceIndex_1 = gl_InstanceIndex;
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e24: u32 = vId;
    let _e26: vec2<f32> = vUV;
    let _e28: vec4<f32> = gl_Position;
    return VertexOutput(_e24, _e26, _e28);
}
