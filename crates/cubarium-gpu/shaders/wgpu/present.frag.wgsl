struct Present {
    col0_: vec4<f32>,
    offset: vec4<f32>,
}

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
}

const MAX_TAPS: i32 = 8i;

@group(0) @binding(32) 
var wgpu_sampler_0_32_: sampler;
@group(0) @binding(0) 
var wgpu_raster: texture_2d<f32>;
var<immediate> p: Present;
var<private> outColour: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;

fn areaAverage(lo: vec2<f32>, hi: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    var lo_1: vec2<f32>;
    var hi_1: vec2<f32>;
    var size_1: vec2<f32>;
    var span: vec2<f32>;
    var over: vec2<f32>;
    var i0_: vec2<i32>;
    var i1_: vec2<i32>;
    var sum: vec3<f32> = vec3(0f);
    var total: f32 = 0f;
    var y: i32;
    var wy: f32;
    var x: i32;
    var wx: f32;

    lo_1 = lo;
    hi_1 = hi;
    size_1 = size;
    let _e15: vec2<f32> = hi_1;
    let _e16: vec2<f32> = lo_1;
    span = (_e15 - _e16);
    let _e19: vec2<f32> = span;
    over = max((_e19 - vec2(7f)), vec2(0f));
    let _e30: vec2<f32> = lo_1;
    let _e32: vec2<f32> = over;
    lo_1 = (_e30 + (0.5f * _e32));
    let _e35: vec2<f32> = hi_1;
    let _e37: vec2<f32> = over;
    hi_1 = (_e35 - (0.5f * _e37));
    let _e40: vec2<f32> = lo_1;
    lo_1 = max(_e40, vec2(0f));
    let _e44: vec2<f32> = hi_1;
    let _e45: vec2<f32> = size_1;
    hi_1 = min(_e44, _e45);
    let _e47: vec2<f32> = lo_1;
    i0_ = vec2<i32>(floor(_e47));
    let _e51: vec2<f32> = hi_1;
    let _e57: vec2<f32> = size_1;
    i1_ = min((vec2<i32>(ceil(_e51)) - vec2(1i)), (vec2<i32>(_e57) - vec2(1i)));
    let _e69: vec2<i32> = i0_;
    y = _e69.y;
    loop {
        let _e72: i32 = y;
        let _e73: vec2<i32> = i1_;
        let _e76: i32 = y;
        let _e77: vec2<i32> = i0_;
        if !(((_e72 <= _e73.y) && (_e76 < (_e77.y + MAX_TAPS)))) {
            break;
        }
        {
            let _e86: vec2<f32> = hi_1;
            let _e88: i32 = y;
            let _e93: vec2<f32> = lo_1;
            let _e95: i32 = y;
            wy = (min(_e86.y, f32((_e88 + 1i))) - max(_e93.y, f32(_e95)));
            let _e100: f32 = wy;
            if (_e100 <= 0.0001f) {
                {
                    continue;
                }
            }
            let _e103: vec2<i32> = i0_;
            x = _e103.x;
            loop {
                let _e106: i32 = x;
                let _e107: vec2<i32> = i1_;
                let _e110: i32 = x;
                let _e111: vec2<i32> = i0_;
                if !(((_e106 <= _e107.x) && (_e110 < (_e111.x + MAX_TAPS)))) {
                    break;
                }
                {
                    let _e120: vec2<f32> = hi_1;
                    let _e122: i32 = x;
                    let _e127: vec2<f32> = lo_1;
                    let _e129: i32 = x;
                    wx = (min(_e120.x, f32((_e122 + 1i))) - max(_e127.x, f32(_e129)));
                    let _e134: f32 = wx;
                    if (_e134 <= 0.0001f) {
                        {
                            continue;
                        }
                    }
                    let _e137: vec3<f32> = sum;
                    let _e138: f32 = wx;
                    let _e139: f32 = wy;
                    let _e141: i32 = x;
                    let _e142: i32 = y;
                    let _e145: vec4<f32> = textureLoad(wgpu_raster, vec2<i32>(_e141, _e142), 0i);
                    sum = (_e137 + ((_e138 * _e139) * _e145.xyz));
                    let _e149: f32 = total;
                    let _e150: f32 = wx;
                    let _e151: f32 = wy;
                    total = (_e149 + (_e150 * _e151));
                }
                continuing {
                    let _e117: i32 = x;
                    x = (_e117 + 1i);
                }
            }
        }
        continuing {
            let _e83: i32 = y;
            y = (_e83 + 1i);
        }
    }
    let _e154: vec3<f32> = sum;
    let _e155: f32 = total;
    let _e159: vec3<f32> = (_e154 / vec3(max(_e155, 0.000001f)));
    let _e160: f32 = total;
    return vec4<f32>(_e159.x, _e159.y, _e159.z, _e160);
}

fn main_1() {
    var panel: vec2<f32>;
    var c: vec4<f32>;
    var size_2: vec2<f32>;
    var step_: vec2<f32>;
    var centre: vec2<f32>;
    var lo_2: vec2<f32>;
    var hi_2: vec2<f32>;
    var src: vec2<f32>;
    var lo_3: vec3<f32>;
    var hi_3: vec3<f32>;

    let _e10: vec4<f32> = gl_FragCoord_1;
    panel = floor(_e10.xy);
    let _e15: Present = p;
    if (_e15.offset.w > 0.5f) {
        {
            let _e21: vec2<u32> = textureDimensions(wgpu_raster, 0i);
            size_2 = vec2<f32>(vec2<i32>(_e21));
            let _e25: Present = p;
            let _e28: Present = p;
            step_ = abs(vec2<f32>(_e25.col0_.x, _e28.col0_.w));
            let _e34: vec2<f32> = step_;
            let _e35: vec2<f32> = panel;
            let _e37: Present = p;
            centre = ((_e34 * _e35) + _e37.offset.xy);
            let _e42: vec2<f32> = centre;
            let _e44: vec2<f32> = step_;
            lo_2 = (_e42 - (0.5f * _e44));
            let _e48: vec2<f32> = centre;
            let _e50: vec2<f32> = step_;
            hi_2 = (_e48 + (0.5f * _e50));
            let _e54: vec2<f32> = hi_2;
            let _e59: vec2<f32> = lo_2;
            let _e60: vec2<f32> = size_2;
            if (any((_e54 <= vec2(0f))) || any((_e59 >= _e60))) {
                {
                    outColour = vec4<f32>(0f, 0f, 0f, 1f);
                    return;
                }
            }
            let _e69: vec2<f32> = lo_2;
            let _e70: vec2<f32> = hi_2;
            let _e71: vec2<f32> = size_2;
            let _e72: vec4<f32> = areaAverage(_e69, _e70, _e71);
            c = _e72;
            let _e73: vec4<f32> = c;
            if (_e73.w <= 0.0001f) {
                {
                    outColour = vec4<f32>(0f, 0f, 0f, 1f);
                    return;
                }
            }
        }
    } else {
        {
            let _e82: Present = p;
            let _e84: vec2<f32> = _e82.col0_.xy;
            let _e85: Present = p;
            let _e87: vec2<f32> = _e85.col0_.zw;
            let _e95: vec2<f32> = panel;
            let _e97: Present = p;
            src = floor(((mat2x2<f32>(vec2<f32>(_e84.x, _e84.y), vec2<f32>(_e87.x, _e87.y)) * _e95) + _e97.offset.xy));
            let _e103: vec2<f32> = src;
            let _e106: vec4<f32> = textureLoad(wgpu_raster, vec2<i32>(_e103), 0i);
            c = _e106;
        }
    }
    let _e107: Present = p;
    if (_e107.offset.z > 0.5f) {
        {
            let _e112: vec4<f32> = c;
            lo_3 = (_e112.xyz * 12.92f);
            let _e118: vec4<f32> = c;
            hi_3 = ((1.055f * pow(max(_e118.xyz, vec3(0f)), vec3(0.41666666f))) - vec3(0.055f));
            let _e133: vec4<f32> = c;
            let _e135: vec3<f32> = hi_3;
            let _e136: vec3<f32> = lo_3;
            let _e137: vec4<f32> = c;
            let _e147: vec3<f32> = clamp(mix(_e135, _e136, step(_e137.xyz, vec3(0.0031308f))), vec3(0f), vec3(1f));
            c.x = _e147.x;
            c.y = _e147.y;
            c.z = _e147.z;
        }
    }
    let _e154: vec4<f32> = c;
    let _e155: vec3<f32> = _e154.xyz;
    outColour = vec4<f32>(_e155.x, _e155.y, _e155.z, 1f);
    return;
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e16: vec4<f32> = outColour;
    return FragmentOutput(_e16);
}
