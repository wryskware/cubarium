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

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
}

@group(0) @binding(32) 
var wgpu_sampler_0_32_: sampler;
@group(0) @binding(0) 
var<uniform> u: Scene;
@group(0) @binding(1) 
var wgpu_fieldA: texture_2d<f32>;
@group(0) @binding(2) 
var wgpu_fieldB: texture_2d<f32>;
var<private> outColour: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;

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
    var px_4: vec2<f32>;
    var w: f32;
    var a: f32;
    var saturation: f32;
    var local_1: f32;
    var pt: f32;
    var base: vec3<f32>;
    var c_1: vec3<f32>;
    var cycle: f32;
    var b: f32;

    let _e34: vec4<f32> = gl_FragCoord_1;
    px_4 = _e34.xy;
    let _e38: vec2<f32> = px_4;
    let _e39: f32 = filteredAt(wgpu_fieldA, 1i, _e38);
    w = _e39;
    let _e41: f32 = w;
    if !((_e41 > 0f)) {
        {
            discard;
        }
    }
    let _e46: f32 = w;
    let _e48: Scene = u;
    a = (1f - exp((-(_e46) / _e48.waterLow.w)));
    let _e55: f32 = a;
    if (_e55 <= 0f) {
        {
            discard;
        }
    }
    let _e58: Scene = u;
    let _e61: Scene = u;
    saturation = (_e58.grid.w * _e61.knobs.x);
    let _e66: f32 = saturation;
    if (_e66 > 0f) {
        let _e70: vec2<f32> = px_4;
        let _e71: vec2<i32> = cellOf(_e70);
        let _e72: f32 = cellValue(wgpu_fieldA, 0i, _e71);
        let _e73: f32 = saturation;
        local_1 = (_e72 / _e73);
    } else {
        local_1 = 0f;
    }
    let _e77: f32 = local_1;
    pt = _e77;
    let _e79: Scene = u;
    let _e82: Scene = u;
    let _e85: f32 = w;
    base = mix(_e79.waterLow.xyz, _e82.waterHigh.xyz, vec3(clamp(_e85, 0f, 1f)));
    let _e92: vec3<f32> = base;
    let _e93: Scene = u;
    let _e96: f32 = pt;
    let _e100: Scene = u;
    c_1 = mix(_e92, _e93.algae.xyz, vec3((clamp(_e96, 0f, 1f) * _e100.algae.w)));
    let _e107: Scene = u;
    let _e110: Scene = u;
    cycle = fract((_e107.time.x / _e110.knobs.z));
    let _e116: Scene = u;
    let _e120: Scene = u;
    let _e124: f32 = cycle;
    let _e126: vec2<f32> = px_4;
    let _e129: f32 = hashPhase(vec2<i32>(floor(_e126)));
    b = (_e116.waterHigh.w * (1f + (_e120.knobs.y * sin(((6.2831855f * _e124) + _e129)))));
    let _e136: vec3<f32> = c_1;
    let _e137: f32 = b;
    let _e138: f32 = a;
    let _e140: vec3<f32> = (_e136 * (_e137 * _e138));
    let _e141: f32 = a;
    outColour = vec4<f32>(_e140.x, _e140.y, _e140.z, _e141);
    return;
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e40: vec4<f32> = outColour;
    return FragmentOutput(_e40);
}
