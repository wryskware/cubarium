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
    var soil: f32;
    var above: f32;
    var c_1: vec3<f32>;
    var saturation: f32;
    var t: f32;
    var ramp: vec3<f32>;
    var b: f32;
    var d: f32;
    var t_1: f32;
    var s: vec3<f32>;
    var b_1: f32;

    let _e34: vec4<f32> = gl_FragCoord_1;
    px_4 = _e34.xy;
    let _e37: vec2<f32> = px_4;
    let _e39: f32 = soilWeight(_e37.y);
    soil = _e39;
    let _e42: f32 = soil;
    above = (1f - _e42);
    let _e45: Scene = u;
    c_1 = _e45.floorColour.xyz;
    let _e49: Scene = u;
    let _e52: Scene = u;
    saturation = (_e49.grid.w * _e52.knobs.x);
    let _e57: f32 = above;
    let _e60: f32 = saturation;
    if ((_e57 > 0f) && (_e60 > 0f)) {
        {
            let _e65: vec2<f32> = px_4;
            let _e66: f32 = filteredAt(wgpu_fieldA, 0i, _e65);
            let _e67: f32 = saturation;
            t = min((_e66 / _e67), 1f);
            let _e72: f32 = t;
            if (_e72 != 0f) {
                {
                    let _e75: Scene = u;
                    let _e78: Scene = u;
                    let _e81: f32 = t;
                    ramp = mix(_e75.producerLow.xyz, _e78.producerHigh.xyz, vec3(_e81));
                    let _e85: Scene = u;
                    let _e88: Scene = u;
                    let _e91: Scene = u;
                    let _e95: f32 = t;
                    let _e97: f32 = t;
                    b = (_e85.producerLow.w + (((_e88.producerHigh.w - _e91.producerLow.w) * _e95) * _e97));
                    let _e101: vec3<f32> = c_1;
                    let _e102: vec3<f32> = ramp;
                    let _e103: f32 = b;
                    let _e104: f32 = above;
                    c_1 = (_e101 + (_e102 * (_e103 * _e104)));
                }
            }
            let _e109: vec2<f32> = px_4;
            let _e110: vec2<i32> = cellOf(_e109);
            let _e111: f32 = cellValue(wgpu_fieldA, 2i, _e110);
            d = _e111;
            let _e113: f32 = d;
            let _e114: Scene = u;
            if (_e113 > _e114.bands.z) {
                {
                    let _e118: vec3<f32> = c_1;
                    let _e119: Scene = u;
                    let _e122: f32 = d;
                    let _e123: Scene = u;
                    let _e129: f32 = above;
                    c_1 = (_e118 + (_e119.detritus.xyz * (min((_e122 / _e123.detritus.w), 1f) * _e129)));
                }
            }
        }
    }
    let _e133: f32 = soil;
    if (_e133 > 0f) {
        {
            let _e137: vec2<f32> = px_4;
            let _e138: f32 = filteredAt(wgpu_fieldA, 2i, _e137);
            let _e139: Scene = u;
            t_1 = clamp((_e138 / _e139.bands.w), 0f, 1f);
            let _e147: Scene = u;
            let _e150: Scene = u;
            let _e153: f32 = t_1;
            s = mix(_e147.soilLow.xyz, _e150.soilHigh.xyz, vec3(_e153));
            let _e157: Scene = u;
            let _e160: Scene = u;
            let _e163: Scene = u;
            let _e167: f32 = t_1;
            let _e170: f32 = soil;
            b_1 = ((_e157.soilLow.w + ((_e160.soilHigh.w - _e163.soilLow.w) * _e167)) * _e170);
            let _e173: vec3<f32> = c_1;
            let _e174: vec3<f32> = s;
            let _e175: f32 = b_1;
            c_1 = (_e173 + (_e174 * _e175));
        }
    }
    let _e178: vec3<f32> = c_1;
    outColour = vec4<f32>(_e178.x, _e178.y, _e178.z, 1f);
    return;
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e40: vec4<f32> = outColour;
    return FragmentOutput(_e40);
}
