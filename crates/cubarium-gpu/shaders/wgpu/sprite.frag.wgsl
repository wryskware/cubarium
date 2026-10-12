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
@group(0) @binding(3) 
var wgpu_atlas: texture_2d<f32>;
@group(0) @binding(4) 
var wgpu_scratch: texture_2d<f32>;
var<private> vPlace_1: vec4<f32>;
var<private> vFrames01_1: vec4<u32>;
var<private> vFrames23_1: vec4<u32>;
var<private> vSizePivot_1: vec4<u32>;
var<private> vWeights_1: vec4<f32>;
var<private> vBend_1: vec4<f32>;
var<private> vMask_1: vec4<f32>;
var<private> vTone_1: vec4<f32>;
var<private> vShade_1: vec4<f32>;
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

fn unit(v: f32) -> f32 {
    var v_1: f32;

    v_1 = v;
    let _e46: f32 = v_1;
    return clamp(_e46, 0f, 1f);
}

fn texelOf(origin: vec2<u32>, texel: vec2<i32>, tile: vec2<f32>, fromScratch: bool) -> vec4<f32> {
    var origin_1: vec2<u32>;
    var texel_1: vec2<i32>;
    var tile_1: vec2<f32>;
    var fromScratch_1: bool;
    var t: vec4<f32>;

    origin_1 = origin;
    texel_1 = texel;
    tile_1 = tile;
    fromScratch_1 = fromScratch;
    let _e52: vec2<i32> = texel_1;
    let _e56: vec2<i32> = texel_1;
    let _e61: vec2<i32> = texel_1;
    let _e63: vec2<f32> = tile_1;
    let _e68: vec2<i32> = texel_1;
    let _e70: vec2<f32> = tile_1;
    if ((((_e52.x < 0i) || (_e56.y < 0i)) || (_e61.x >= i32(_e63.x))) || (_e68.y >= i32(_e70.y))) {
        {
            return vec4(0f);
        }
    }
    let _e77: bool = fromScratch_1;
    if _e77 {
        {
            let _e78: vec2<u32> = origin_1;
            let _e80: vec2<i32> = texel_1;
            let _e83: vec4<f32> = textureLoad(wgpu_scratch, (vec2<i32>(_e78) + _e80), 0i);
            return _e83;
        }
    }
    let _e84: vec2<u32> = origin_1;
    let _e86: vec2<i32> = texel_1;
    let _e89: vec4<f32> = textureLoad(wgpu_atlas, (vec2<i32>(_e84) + _e86), 0i);
    t = _e89;
    let _e91: vec4<f32> = t;
    let _e93: vec4<f32> = t;
    let _e95: vec3<f32> = (_e91.xyz * _e93.w);
    let _e96: vec4<f32> = t;
    return vec4<f32>(_e95.x, _e95.y, _e95.z, _e96.w);
}

fn frameAt(origin_2: vec2<u32>, src: vec2<f32>, tile_2: vec2<f32>, weight_1: f32, fromScratch_2: bool) -> vec4<f32> {
    var origin_3: vec2<u32>;
    var src_1: vec2<f32>;
    var tile_3: vec2<f32>;
    var weight_2: f32;
    var fromScratch_3: bool;
    var p_2: vec2<f32>;
    var t0_: vec2<i32>;
    var fr: vec2<f32>;
    var sum_1: vec4<f32>;

    origin_3 = origin_2;
    src_1 = src;
    tile_3 = tile_2;
    weight_2 = weight_1;
    fromScratch_3 = fromScratch_2;
    let _e54: f32 = weight_2;
    if (_e54 <= 0f) {
        {
            return vec4(0f);
        }
    }
    let _e59: Scene = u;
    if (_e59.time.z > 0.5f) {
        {
            let _e64: vec2<f32> = src_1;
            p_2 = (_e64 - vec2(0.5f));
            let _e69: vec2<f32> = p_2;
            t0_ = vec2<i32>(floor(_e69));
            let _e73: vec2<f32> = p_2;
            let _e74: vec2<i32> = t0_;
            fr = (_e73 - vec2<f32>(_e74));
            let _e78: vec2<u32> = origin_3;
            let _e79: vec2<i32> = t0_;
            let _e80: vec2<f32> = tile_3;
            let _e81: bool = fromScratch_3;
            let _e82: vec4<f32> = texelOf(_e78, _e79, _e80, _e81);
            let _e84: vec2<f32> = fr;
            let _e88: vec2<f32> = fr;
            let _e93: vec2<u32> = origin_3;
            let _e94: vec2<i32> = t0_;
            let _e99: vec2<f32> = tile_3;
            let _e100: bool = fromScratch_3;
            let _e101: vec4<f32> = texelOf(_e93, (_e94 + vec2<i32>(1i, 0i)), _e99, _e100);
            let _e102: vec2<f32> = fr;
            let _e105: vec2<f32> = fr;
            let _e111: vec2<u32> = origin_3;
            let _e112: vec2<i32> = t0_;
            let _e117: vec2<f32> = tile_3;
            let _e118: bool = fromScratch_3;
            let _e119: vec4<f32> = texelOf(_e111, (_e112 + vec2<i32>(0i, 1i)), _e117, _e118);
            let _e121: vec2<f32> = fr;
            let _e124: vec2<f32> = fr;
            let _e129: vec2<u32> = origin_3;
            let _e130: vec2<i32> = t0_;
            let _e135: vec2<f32> = tile_3;
            let _e136: bool = fromScratch_3;
            let _e137: vec4<f32> = texelOf(_e129, (_e130 + vec2<i32>(1i, 1i)), _e135, _e136);
            let _e138: vec2<f32> = fr;
            let _e140: vec2<f32> = fr;
            sum_1 = ((((_e82 * ((1f - _e84.x) * (1f - _e88.y))) + (_e101 * (_e102.x * (1f - _e105.y)))) + (_e119 * ((1f - _e121.x) * _e124.y))) + (_e137 * (_e138.x * _e140.y)));
            let _e146: vec4<f32> = sum_1;
            let _e147: f32 = weight_2;
            return (_e146 * _e147);
        }
    }
    let _e149: vec2<u32> = origin_3;
    let _e150: vec2<f32> = src_1;
    let _e153: vec2<f32> = tile_3;
    let _e154: bool = fromScratch_3;
    let _e155: vec4<f32> = texelOf(_e149, vec2<i32>(floor(_e150)), _e153, _e154);
    let _e156: f32 = weight_2;
    return (_e155 * _e156);
}

fn main_1() {
    var pivot: vec2<f32>;
    var tile_4: vec2<f32>;
    var offset: vec2<f32>;
    var heading: vec2<f32>;
    var vLocal: vec2<f32>;
    var py: f32;
    var d: f32 = 0f;
    var t_1: f32;
    var src_2: vec2<f32>;
    var cover: f32;
    var r: f32;
    var h_2: f32;
    var opacity: f32;
    var fromScratch_4: bool;
    var rgba: vec4<f32>;
    var luma: f32;
    var local_1: f32;
    var lit: f32;
    var s: f32;

    let _e44: vec4<u32> = vSizePivot_1;
    pivot = vec2<f32>(_e44.zw);
    let _e48: vec4<u32> = vSizePivot_1;
    tile_4 = vec2<f32>(_e48.xy);
    let _e53: vec4<f32> = gl_FragCoord_1;
    let _e55: vec4<f32> = vPlace_1;
    let _e58: Scene = u;
    let _e61: vec4<f32> = vShade_1;
    offset = ((_e53.xy - _e55.xy) / vec2((_e58.time.w * max(_e61.z, 0.001f))));
    let _e69: vec4<f32> = vPlace_1;
    heading = _e69.zw;
    let _e72: vec2<f32> = offset;
    let _e73: vec2<f32> = heading;
    let _e75: vec2<f32> = offset;
    let _e76: vec2<f32> = heading;
    let _e79: vec2<f32> = heading;
    vLocal = vec2<f32>(dot(_e72, _e73), dot(_e75, vec2<f32>(-(_e76.y), _e79.x)));
    let _e85: vec2<f32> = vLocal;
    let _e87: vec2<f32> = pivot;
    py = (_e85.y + _e87.y);
    let _e93: vec4<f32> = vBend_1;
    let _e97: vec4<f32> = vBend_1;
    if ((_e93.x != 0f) && (_e97.w > 0f)) {
        {
            let _e102: vec2<f32> = tile_4;
            let _e104: f32 = py;
            let _e106: vec4<f32> = vBend_1;
            let _e109: vec4<f32> = vBend_1;
            let _e112: vec4<f32> = vBend_1;
            t_1 = clamp(((((_e102.y - _e104) + _e106.y) - _e109.z) / _e112.w), 0f, 1f);
            let _e119: vec4<f32> = vBend_1;
            let _e121: f32 = t_1;
            let _e122: f32 = t_1;
            let _e126: f32 = t_1;
            d = (_e119.x * ((_e121 * _e122) * (3f - (2f * _e126))));
            let _e131: Scene = u;
            if (_e131.knobs.w < 0.5f) {
                {
                    let _e136: f32 = d;
                    d = round(_e136);
                }
            }
        }
    }
    let _e138: vec2<f32> = vLocal;
    let _e140: f32 = d;
    let _e142: vec2<f32> = vLocal;
    let _e145: vec2<f32> = pivot;
    src_2 = (vec2<f32>((_e138.x - _e140), _e142.y) + _e145);
    let _e148: Scene = u;
    let _e153: vec2<f32> = src_2;
    let _e157: vec2<f32> = src_2;
    let _e162: vec2<f32> = src_2;
    let _e164: vec2<f32> = tile_4;
    let _e168: vec2<f32> = src_2;
    let _e170: vec2<f32> = tile_4;
    if ((_e148.time.z <= 0.5f) && ((((_e153.x < 0f) || (_e157.y < 0f)) || (_e162.x >= _e164.x)) || (_e168.y >= _e170.y))) {
        {
            discard;
        }
    }
    let _e176: vec4<f32> = vMask_1;
    if (_e176.z > 0.5f) {
        {
            let _e180: vec2<f32> = src_2;
            let _e181: vec2<f32> = pivot;
            r = length((_e180 - _e181));
            let _e185: vec4<f32> = vMask_1;
            let _e187: f32 = r;
            let _e191: f32 = unit(((_e185.y - _e187) + 0.5f));
            let _e192: vec4<f32> = vMask_1;
            let _e194: f32 = unit(_e192.y);
            cover = (_e191 * _e194);
        }
    } else {
        {
            let _e196: vec2<f32> = tile_4;
            let _e198: vec2<f32> = src_2;
            h_2 = (_e196.y - _e198.y);
            let _e202: vec4<f32> = vMask_1;
            let _e204: f32 = h_2;
            let _e208: f32 = unit(((_e202.y - _e204) + 0.5f));
            let _e209: f32 = h_2;
            let _e210: vec4<f32> = vMask_1;
            let _e215: f32 = unit(((_e209 - _e210.x) + 0.5f));
            let _e217: vec4<f32> = vMask_1;
            let _e219: vec4<f32> = vMask_1;
            let _e224: f32 = unit((_e217.y - max(_e219.x, 0f)));
            cover = ((_e208 * _e215) * _e224);
        }
    }
    let _e226: vec4<f32> = vMask_1;
    opacity = _e226.w;
    let _e229: f32 = cover;
    let _e232: f32 = opacity;
    if ((_e229 <= 0f) || (_e232 <= 0f)) {
        {
            discard;
        }
    }
    let _e236: vec4<f32> = vShade_1;
    fromScratch_4 = (_e236.w > 0.5f);
    let _e241: vec4<u32> = vFrames01_1;
    let _e243: vec2<f32> = src_2;
    let _e244: vec2<f32> = tile_4;
    let _e245: vec4<f32> = vWeights_1;
    let _e247: bool = fromScratch_4;
    let _e248: vec4<f32> = frameAt(_e241.xy, _e243, _e244, _e245.x, _e247);
    let _e249: vec4<u32> = vFrames01_1;
    let _e251: vec2<f32> = src_2;
    let _e252: vec2<f32> = tile_4;
    let _e253: vec4<f32> = vWeights_1;
    let _e255: bool = fromScratch_4;
    let _e256: vec4<f32> = frameAt(_e249.zw, _e251, _e252, _e253.y, _e255);
    let _e258: vec4<u32> = vFrames23_1;
    let _e260: vec2<f32> = src_2;
    let _e261: vec2<f32> = tile_4;
    let _e262: vec4<f32> = vWeights_1;
    let _e264: bool = fromScratch_4;
    let _e265: vec4<f32> = frameAt(_e258.xy, _e260, _e261, _e262.z, _e264);
    let _e267: vec4<u32> = vFrames23_1;
    let _e269: vec2<f32> = src_2;
    let _e270: vec2<f32> = tile_4;
    let _e271: vec4<f32> = vWeights_1;
    let _e273: bool = fromScratch_4;
    let _e274: vec4<f32> = frameAt(_e267.zw, _e269, _e270, _e271.w, _e273);
    rgba = (((_e248 + _e256) + _e265) + _e274);
    let _e277: vec4<f32> = rgba;
    let _e278: f32 = cover;
    rgba = (_e277 * _e278);
    let _e280: vec4<f32> = vTone_1;
    let _e284: vec4<f32> = rgba;
    if ((_e280.w > 0f) && (_e284.w > 0f)) {
        {
            let _e289: vec4<f32> = rgba;
            let _e296: vec4<f32> = rgba;
            luma = (dot(_e289.xyz, vec3<f32>(0.2126f, 0.7152f, 0.0722f)) / _e296.w);
            let _e300: vec4<f32> = vShade_1;
            if (_e300.y > 0f) {
                let _e304: f32 = luma;
                let _e305: vec4<f32> = vShade_1;
                local_1 = clamp((_e304 / _e305.y), 0f, 1f);
            } else {
                local_1 = 1f;
            }
            let _e313: f32 = local_1;
            lit = _e313;
            let _e315: vec4<f32> = vShade_1;
            let _e318: vec4<f32> = vShade_1;
            let _e321: f32 = lit;
            let _e324: vec4<f32> = rgba;
            s = ((_e315.x + ((1f - _e318.x) * _e321)) * _e324.w);
            let _e328: vec4<f32> = rgba;
            let _e330: vec4<f32> = rgba;
            let _e332: vec4<f32> = vTone_1;
            let _e334: f32 = s;
            let _e336: vec4<f32> = rgba;
            let _e339: vec4<f32> = vTone_1;
            let _e342: vec3<f32> = (_e330.xyz + (((_e332.xyz * _e334) - _e336.xyz) * _e339.w));
            rgba.x = _e342.x;
            rgba.y = _e342.y;
            rgba.z = _e342.z;
        }
    }
    let _e349: vec4<f32> = rgba;
    let _e351: f32 = opacity;
    if ((_e349.w * _e351) <= 0f) {
        {
            discard;
        }
    }
    let _e355: vec4<f32> = rgba;
    let _e356: f32 = opacity;
    outColour = (_e355 * _e356);
    return;
}

@fragment 
fn main(@location(0) @interpolate(flat) vPlace: vec4<f32>, @location(1) @interpolate(flat) vFrames01_: vec4<u32>, @location(2) @interpolate(flat) vFrames23_: vec4<u32>, @location(3) @interpolate(flat) vSizePivot: vec4<u32>, @location(4) @interpolate(flat) vWeights: vec4<f32>, @location(5) @interpolate(flat) vBend: vec4<f32>, @location(6) @interpolate(flat) vMask: vec4<f32>, @location(7) @interpolate(flat) vTone: vec4<f32>, @location(8) @interpolate(flat) vShade: vec4<f32>, @builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    vPlace_1 = vPlace;
    vFrames01_1 = vFrames01_;
    vFrames23_1 = vFrames23_;
    vSizePivot_1 = vSizePivot;
    vWeights_1 = vWeights;
    vBend_1 = vBend;
    vMask_1 = vMask;
    vTone_1 = vTone;
    vShade_1 = vShade;
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e80: vec4<f32> = outColour;
    return FragmentOutput(_e80);
}
