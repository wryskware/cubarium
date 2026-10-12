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
    @location(0) @interpolate(flat) vFig: i32,
    @builtin(position) gl_Position: vec4<f32>,
}

const FACE_O: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, 0f), vec3<f32>(1f, 0f, 1f));
const FACE_U: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(-1f, 0f, 0f));
const FACE_V: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 1f, 0f));
const FACE_N: array<vec3<f32>, 6> = array<vec3<f32>, 6>(vec3<f32>(-1f, 0f, 0f), vec3<f32>(1f, 0f, 0f), vec3<f32>(0f, -1f, 0f), vec3<f32>(0f, 1f, 0f), vec3<f32>(0f, 0f, -1f), vec3<f32>(0f, 0f, 1f));

var<immediate> pc: FigurePush;
@group(1) @binding(0) 
var<storage> global: FigureVoxels;
@group(1) @binding(1) 
var<storage> global_1: FigureTiles;
@group(1) @binding(2) 
var<storage> global_2: FigureFigs;
@group(1) @binding(4) 
var<storage> global_3: FigureBones;
var<private> vFig: i32;
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

fn main_1() {
    var f: Fig;
    var a: Tile;
    var lo: vec2<f32>;
    var hi: vec2<f32>;
    var b: Tile;
    var corner: vec2<f32>;
    var px: vec2<f32>;

    let _e19: u32 = gl_InstanceIndex_1;
    let _e22: Fig = global_2.figs[i32(_e19)];
    f = _e22;
    let _e24: Fig = f;
    let _e28: Tile = global_1.tiles[_e24.tiles.x];
    a = _e28;
    let _e30: Tile = a;
    lo = vec2<f32>(_e30.orig.xy);
    let _e35: vec2<f32> = lo;
    let _e36: Tile = a;
    let _e40: Tile = a;
    hi = (_e35 + (vec2<f32>(_e36.rect.zw) / vec2(f32(_e40.faceSize.w))));
    let _e48: Fig = f;
    if (_e48.tiles.y >= 0i) {
        {
            let _e53: Fig = f;
            let _e57: Tile = global_1.tiles[_e53.tiles.y];
            b = _e57;
            let _e59: vec2<f32> = lo;
            let _e60: Tile = b;
            lo = min(_e59, vec2<f32>(_e60.orig.xy));
            let _e65: vec2<f32> = hi;
            let _e66: Tile = b;
            let _e70: Tile = b;
            let _e74: Tile = b;
            hi = max(_e65, (vec2<f32>(_e66.orig.xy) + (vec2<f32>(_e70.rect.zw) / vec2(f32(_e74.faceSize.w)))));
        }
    }
    let _e82: Fig = f;
    let _e85: vec2<f32> = lo;
    lo = (floor((_e82.anchor.xy + _e85)) - vec2(1f));
    let _e91: Fig = f;
    let _e94: vec2<f32> = hi;
    hi = (ceil((_e91.anchor.xy + _e94)) + vec2(1f));
    let _e101: u32 = gl_VertexIndex_1;
    let _e106: u32 = gl_VertexIndex_1;
    corner = vec2<f32>(f32((i32(_e101) & 1i)), f32(((i32(_e106) >> 1u) & 1i)));
    let _e116: vec2<f32> = lo;
    let _e117: vec2<f32> = hi;
    let _e118: vec2<f32> = corner;
    px = mix(_e116, _e117, _e118);
    let _e122: vec2<f32> = px;
    let _e123: FigurePush = pc;
    let _e127: FigurePush = pc;
    let _e137: vec2<f32> = (((_e122 / vec2<f32>(f32(_e123.k.x), f32(_e127.k.w))) * 2f) - vec2(1f));
    gl_Position = vec4<f32>(_e137.x, _e137.y, 0f, 1f);
    let _e144: vec4<f32> = gl_Position;
    gl_Position.y = -(_e144.y);
    let _e147: u32 = gl_InstanceIndex_1;
    vFig = i32(_e147);
    return;
}

@vertex 
fn main(@builtin(instance_index) gl_InstanceIndex: u32, @builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    gl_InstanceIndex_1 = gl_InstanceIndex;
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e28: i32 = vFig;
    let _e30: vec4<f32> = gl_Position;
    return VertexOutput(_e28, _e30);
}
