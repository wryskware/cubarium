struct Scene {
    raster: vec4<f32>,
    grid: vec4<f32>,
    time: vec4<f32>,
    floorColour: vec4<f32>,
    producerLow: vec4<f32>,
    producerHigh: vec4<f32>,
    detritus: vec4<f32>,
    soilLow: vec4<f32>,
    soilHigh: vec4<f32>,
    waterLow: vec4<f32>,
    waterHigh: vec4<f32>,
    algae: vec4<f32>,
    bands: vec4<f32>,
    knobs: vec4<f32>,
}

struct VertexOutput {
    @location(0) @interpolate(flat) vPlace: vec4<f32>,
    @location(1) @interpolate(flat) vFrames01_: vec4<u32>,
    @location(2) @interpolate(flat) vFrames23_: vec4<u32>,
    @location(3) @interpolate(flat) vSizePivot: vec4<u32>,
    @location(4) @interpolate(flat) vWeights: vec4<f32>,
    @location(5) @interpolate(flat) vBend: vec4<f32>,
    @location(6) @interpolate(flat) vMask: vec4<f32>,
    @location(7) @interpolate(flat) vTone: vec4<f32>,
    @location(8) @interpolate(flat) vShade: vec4<f32>,
    @builtin(position) gl_Position: vec4<f32>,
}

@group(0) @binding(32) 
var wgpu_sampler_0_32_: sampler;
@group(0) @binding(0) 
var<uniform> u: Scene;
@group(0) @binding(1) 
var wgpu_fieldA: texture_2d<f32>;
@group(0) @binding(2) 
var wgpu_fieldB: texture_2d<f32>;
var<private> iAnchor_1: vec2<f32>;
var<private> iHeading_1: vec2<f32>;
var<private> iFrames01_1: vec4<u32>;
var<private> iFrames23_1: vec4<u32>;
var<private> iSizePivot_1: vec4<u32>;
var<private> iWeights_1: vec4<f32>;
var<private> iBend_1: vec4<f32>;
var<private> iMask_1: vec4<f32>;
var<private> iTone_1: vec4<f32>;
var<private> iShade_1: vec4<f32>;
var<private> iBox_1: vec4<f32>;
var<private> vPlace: vec4<f32>;
var<private> vFrames01_: vec4<u32>;
var<private> vFrames23_: vec4<u32>;
var<private> vSizePivot: vec4<u32>;
var<private> vWeights: vec4<f32>;
var<private> vBend: vec4<f32>;
var<private> vMask: vec4<f32>;
var<private> vTone: vec4<f32>;
var<private> vShade: vec4<f32>;
var<private> gl_VertexIndex_1: u32;
var<private> gl_Position: vec4<f32>;

fn cellOf(px: vec2<f32>) -> vec2<i32> {
    var px_1: vec2<f32>;
    var cell: f32;

    px_1 = px;
    let _e35: Scene = u;
    cell = (4f * _e35.grid.z);
    let _e40: vec2<f32> = px_1;
    let _e41: f32 = cell;
    return vec2<i32>(floor((_e40 / vec2(_e41))));
}

fn cellValue(field: texture_2d<f32>, channel: i32, cell_1: vec2<i32>) -> f32 {
    var channel_1: i32;
    var cell_2: vec2<i32>;
    var nx: i32;
    var ny: i32;
    var local: i32;
    var cx: i32;
    var c: vec2<i32>;

    channel_1 = channel;
    cell_2 = cell_1;
    let _e37: Scene = u;
    nx = i32(_e37.grid.x);
    let _e42: Scene = u;
    ny = i32(_e42.grid.y);
    let _e47: vec2<i32> = cell_2;
    if (_e47.x >= 0i) {
        let _e51: vec2<i32> = cell_2;
        let _e53: i32 = nx;
        local = (_e51.x % _e53);
    } else {
        let _e55: i32 = nx;
        let _e60: vec2<i32> = cell_2;
        let _e63: i32 = nx;
        local = ((_e55 - 1i) - ((-1i - _e60.x) % _e63));
    }
    let _e67: i32 = local;
    cx = _e67;
    let _e69: i32 = cx;
    let _e70: vec2<i32> = cell_2;
    let _e73: i32 = ny;
    c = vec2<i32>(_e69, clamp(_e70.y, 0i, (_e73 - 1i)));
    let _e79: i32 = channel_1;
    let _e80: vec2<i32> = c;
    let _e82: vec4<f32> = textureLoad(field, _e80, 0i);
    return _e82[_e79];
}

fn filteredAt(field_1: texture_2d<f32>, channel_2: i32, px_2: vec2<f32>) -> f32 {
    var channel_3: i32;
    var px_3: vec2<f32>;
    var sum: f32;
    var weight: f32 = 4f;

    channel_3 = channel_2;
    px_3 = px_2;
    let _e37: i32 = channel_3;
    let _e38: vec2<f32> = px_3;
    let _e39: vec2<i32> = cellOf(_e38);
    let _e40: f32 = cellValue(field_1, _e37, _e39);
    sum = (_e40 * 4f);
    let _e46: f32 = sum;
    let _e47: i32 = channel_3;
    let _e48: vec2<f32> = px_3;
    let _e53: vec2<i32> = cellOf((_e48 + vec2<f32>(1f, 0f)));
    let _e54: f32 = cellValue(field_1, _e47, _e53);
    sum = (_e46 + _e54);
    let _e56: f32 = sum;
    let _e57: i32 = channel_3;
    let _e58: vec2<f32> = px_3;
    let _e63: vec2<i32> = cellOf((_e58 - vec2<f32>(1f, 0f)));
    let _e64: f32 = cellValue(field_1, _e57, _e63);
    sum = (_e56 + _e64);
    let _e66: f32 = weight;
    weight = (_e66 + 2f);
    let _e69: vec2<f32> = px_3;
    let _e73: Scene = u;
    if ((_e69.y + 1f) < _e73.raster.y) {
        {
            let _e77: f32 = sum;
            let _e78: i32 = channel_3;
            let _e79: vec2<f32> = px_3;
            let _e84: vec2<i32> = cellOf((_e79 + vec2<f32>(0f, 1f)));
            let _e85: f32 = cellValue(field_1, _e78, _e84);
            sum = (_e77 + _e85);
            let _e87: f32 = weight;
            weight = (_e87 + 1f);
        }
    }
    let _e90: vec2<f32> = px_3;
    if ((_e90.y - 1f) >= 0f) {
        {
            let _e96: f32 = sum;
            let _e97: i32 = channel_3;
            let _e98: vec2<f32> = px_3;
            let _e103: vec2<i32> = cellOf((_e98 - vec2<f32>(0f, 1f)));
            let _e104: f32 = cellValue(field_1, _e97, _e103);
            sum = (_e96 + _e104);
            let _e106: f32 = weight;
            weight = (_e106 + 1f);
        }
    }
    let _e109: f32 = sum;
    let _e110: f32 = weight;
    return (_e109 / _e110);
}

fn heightAt(y: f32) -> f32 {
    var y_1: f32;

    y_1 = y;
    let _e36: f32 = y_1;
    let _e37: Scene = u;
    return (1f - (2f * (_e36 / _e37.raster.y)));
}

fn soilWeight(y_2: f32) -> f32 {
    var y_3: f32;
    var h: f32;

    y_3 = y_2;
    let _e34: f32 = y_3;
    let _e35: f32 = heightAt(_e34);
    h = _e35;
    let _e38: Scene = u;
    let _e41: Scene = u;
    let _e45: Scene = u;
    let _e48: Scene = u;
    let _e52: f32 = h;
    return (1f - smoothstep((_e38.bands.x - _e41.bands.y), (_e45.bands.x + _e48.bands.y), _e52));
}

fn hashPhase(p: vec2<i32>) -> f32 {
    var p_1: vec2<i32>;
    var h_1: u32;

    p_1 = p;
    let _e34: vec2<i32> = p_1;
    let _e39: vec2<i32> = p_1;
    h_1 = ((u32(_e34.x) * 2654435769u) ^ (u32(_e39.y) * 2246822507u));
    let _e46: u32 = h_1;
    let _e47: u32 = h_1;
    h_1 = (_e46 ^ (_e47 >> 16u));
    let _e52: u32 = h_1;
    h_1 = (_e52 * 2146121005u);
    let _e55: u32 = h_1;
    let _e56: u32 = h_1;
    h_1 = (_e55 ^ (_e56 >> 15u));
    let _e61: u32 = h_1;
    h_1 = (_e61 * 2221713035u);
    let _e64: u32 = h_1;
    let _e65: u32 = h_1;
    h_1 = (_e64 ^ (_e65 >> 16u));
    let _e70: u32 = h_1;
    return ((f32((_e70 & 16777215u)) / 16777216f) * 6.2831855f);
}

fn main_1() {
    var pivot: vec2<f32>;
    var tile: vec2<f32>;
    var b0_: vec2<f32>;
    var b1_: vec2<f32>;
    var r: f32;
    var EPS: f32 = 0.05f;
    var filtered: bool;
    var local_1: f32;
    var local_2: f32;
    var reach: f32;
    var local_3: f32;
    var padX: f32;
    var local_4: f32;
    var padY: f32;
    var lo: vec2<f32>;
    var hi: vec2<f32>;
    var stampScale: f32;
    var local_5: f32;
    var local_6: f32;
    var corner: vec2<f32>;
    var local_7: vec2<f32>;
    var anchor: vec2<f32>;
    var side: vec2<f32>;
    var px_4: vec2<f32>;

    let _e52: vec4<u32> = iSizePivot_1;
    pivot = vec2<f32>(_e52.zw);
    let _e56: vec4<u32> = iSizePivot_1;
    tile = vec2<f32>(_e56.xy);
    let _e60: vec4<f32> = iBox_1;
    b0_ = _e60.xy;
    let _e63: vec4<f32> = iBox_1;
    b1_ = _e63.zw;
    let _e66: vec2<f32> = b1_;
    let _e68: vec2<f32> = b0_;
    let _e71: vec2<f32> = b1_;
    let _e73: vec2<f32> = b0_;
    if !(((_e66.x > _e68.x) && (_e71.y > _e73.y))) {
        {
            b0_ = vec2(0f);
            let _e80: vec2<f32> = tile;
            b1_ = _e80;
        }
    }
    let _e81: vec4<f32> = iMask_1;
    if (_e81.z > 0.5f) {
        {
            let _e85: vec4<f32> = iMask_1;
            r = (max(_e85.y, 0f) + 0.5f);
            let _e92: vec2<f32> = b0_;
            let _e93: vec2<f32> = pivot;
            let _e94: f32 = r;
            b0_ = max(_e92, (_e93 - vec2(_e94)));
            let _e98: vec2<f32> = b1_;
            let _e99: vec2<f32> = pivot;
            let _e100: f32 = r;
            b1_ = min(_e98, (_e99 + vec2(_e100)));
        }
    } else {
        {
            let _e105: vec2<f32> = b0_;
            let _e107: vec2<f32> = tile;
            let _e109: vec4<f32> = iMask_1;
            b0_.y = max(_e105.y, ((_e107.y - _e109.y) - 0.5f));
            let _e116: vec2<f32> = b1_;
            let _e118: vec2<f32> = tile;
            let _e120: vec4<f32> = iMask_1;
            b1_.y = min(_e116.y, ((_e118.y - _e120.x) + 0.5f));
        }
    }
    let _e128: Scene = u;
    filtered = (_e128.time.z > 0.5f);
    let _e134: vec4<f32> = iBend_1;
    let _e138: vec4<f32> = iBend_1;
    if ((_e134.x != 0f) && (_e138.w > 0f)) {
        let _e143: vec4<f32> = iBend_1;
        let _e146: Scene = u;
        if (_e146.knobs.w < 0.5f) {
            local_1 = 0.5f;
        } else {
            local_1 = 0f;
        }
        let _e154: f32 = local_1;
        local_2 = (abs(_e143.x) + _e154);
    } else {
        local_2 = 0f;
    }
    let _e158: f32 = local_2;
    reach = _e158;
    let _e160: f32 = reach;
    let _e161: bool = filtered;
    if _e161 {
        local_3 = 1f;
    } else {
        let _e163: f32 = EPS;
        local_3 = _e163;
    }
    let _e165: f32 = local_3;
    padX = (_e160 + _e165);
    let _e168: bool = filtered;
    if _e168 {
        local_4 = 1f;
    } else {
        let _e170: f32 = EPS;
        local_4 = _e170;
    }
    let _e172: f32 = local_4;
    padY = _e172;
    let _e174: vec2<f32> = b0_;
    let _e176: vec2<f32> = pivot;
    let _e179: f32 = padX;
    let _e181: vec2<f32> = b0_;
    let _e183: vec2<f32> = pivot;
    let _e186: f32 = padY;
    lo = vec2<f32>(((_e174.x - _e176.x) - _e179), ((_e181.y - _e183.y) - _e186));
    let _e190: vec2<f32> = b1_;
    let _e192: vec2<f32> = pivot;
    let _e195: f32 = padX;
    let _e197: vec2<f32> = b1_;
    let _e199: vec2<f32> = pivot;
    let _e202: f32 = padY;
    hi = vec2<f32>(((_e190.x - _e192.x) + _e195), ((_e197.y - _e199.y) + _e202));
    let _e206: Scene = u;
    let _e209: vec4<f32> = iShade_1;
    stampScale = (_e206.time.w * max(_e209.z, 0.001f));
    let _e216: u32 = gl_VertexIndex_1;
    if ((i32(_e216) & 1i) == 0i) {
        let _e222: vec2<f32> = lo;
        local_5 = _e222.x;
    } else {
        let _e224: vec2<f32> = hi;
        local_5 = _e224.x;
    }
    let _e227: f32 = local_5;
    let _e228: u32 = gl_VertexIndex_1;
    if ((i32(_e228) & 2i) == 0i) {
        let _e234: vec2<f32> = lo;
        local_6 = _e234.y;
    } else {
        let _e236: vec2<f32> = hi;
        local_6 = _e236.y;
    }
    let _e239: f32 = local_6;
    corner = vec2<f32>(_e227, _e239);
    let _e242: Scene = u;
    if (_e242.time.z > 0.5f) {
        let _e247: vec2<f32> = iAnchor_1;
        local_7 = _e247;
    } else {
        let _e248: vec2<f32> = iAnchor_1;
        local_7 = (floor(_e248) + vec2(0.5f));
    }
    let _e254: vec2<f32> = local_7;
    anchor = _e254;
    let _e256: vec2<f32> = iHeading_1;
    let _e259: vec2<f32> = iHeading_1;
    side = vec2<f32>(-(_e256.y), _e259.x);
    let _e263: vec2<f32> = anchor;
    let _e264: f32 = stampScale;
    let _e265: vec2<f32> = corner;
    let _e267: vec2<f32> = iHeading_1;
    let _e269: vec2<f32> = corner;
    let _e271: vec2<f32> = side;
    px_4 = (_e263 + (_e264 * ((_e265.x * _e267) + (_e269.y * _e271))));
    let _e277: vec2<f32> = anchor;
    let _e278: vec2<f32> = iHeading_1;
    vPlace = vec4<f32>(_e277.x, _e277.y, _e278.x, _e278.y);
    let _e284: vec4<u32> = iFrames01_1;
    vFrames01_ = _e284;
    let _e285: vec4<u32> = iFrames23_1;
    vFrames23_ = _e285;
    let _e286: vec4<u32> = iSizePivot_1;
    vSizePivot = _e286;
    let _e287: vec4<f32> = iWeights_1;
    vWeights = _e287;
    let _e288: vec4<f32> = iBend_1;
    vBend = _e288;
    let _e289: vec4<f32> = iMask_1;
    vMask = _e289;
    let _e290: vec4<f32> = iTone_1;
    vTone = _e290;
    let _e291: vec4<f32> = iShade_1;
    vShade = _e291;
    let _e293: vec2<f32> = px_4;
    let _e294: Scene = u;
    let _e302: vec2<f32> = (((_e293 * _e294.raster.zw) * 2f) - vec2(1f));
    gl_Position = vec4<f32>(_e302.x, _e302.y, 0f, 1f);
    let _e309: vec4<f32> = gl_Position;
    gl_Position.y = -(_e309.y);
    return;
}

@vertex 
fn main(@location(0) iAnchor: vec2<f32>, @location(1) iHeading: vec2<f32>, @location(2) iFrames01_: vec4<u32>, @location(3) iFrames23_: vec4<u32>, @location(4) iSizePivot: vec4<u32>, @location(5) iWeights: vec4<f32>, @location(6) iBend: vec4<f32>, @location(7) iMask: vec4<f32>, @location(8) iTone: vec4<f32>, @location(9) iShade: vec4<f32>, @location(10) iBox: vec4<f32>, @builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    iAnchor_1 = iAnchor;
    iHeading_1 = iHeading;
    iFrames01_1 = iFrames01_;
    iFrames23_1 = iFrames23_;
    iSizePivot_1 = iSizePivot;
    iWeights_1 = iWeights;
    iBend_1 = iBend;
    iMask_1 = iMask;
    iTone_1 = iTone;
    iShade_1 = iShade;
    iBox_1 = iBox;
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e100: vec4<f32> = vPlace;
    let _e102: vec4<u32> = vFrames01_;
    let _e104: vec4<u32> = vFrames23_;
    let _e106: vec4<u32> = vSizePivot;
    let _e108: vec4<f32> = vWeights;
    let _e110: vec4<f32> = vBend;
    let _e112: vec4<f32> = vMask;
    let _e114: vec4<f32> = vTone;
    let _e116: vec4<f32> = vShade;
    let _e118: vec4<f32> = gl_Position;
    return VertexOutput(_e100, _e102, _e104, _e106, _e108, _e110, _e112, _e114, _e116, _e118);
}
