struct Bloom {
    a: vec4<i32>,
    b: vec4<i32>,
    k: vec4<f32>,
}

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
}

@group(0) @binding(32) 
var wgpu_sampler_0_32_: sampler;
@group(0) @binding(33) 
var wgpu_sampler_0_33_: sampler;
var<immediate> pc: Bloom;
@group(0) @binding(0) 
var wgpu_src: texture_2d<f32>;
@group(0) @binding(1) 
var wgpu_emitTex: texture_2d<f32>;
var<private> outColour: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;

fn borderTexel(p: vec2<i32>) -> vec3<f32> {
    var p_1: vec2<i32>;
    var size: vec2<i32>;
    var local: i32;

    p_1 = p;
    let _e15: vec2<u32> = textureDimensions(wgpu_src, 0i);
    size = vec2<i32>(_e15);
    let _e18: vec2<i32> = p_1;
    let _e22: vec2<i32> = p_1;
    let _e24: vec2<i32> = size;
    if ((_e18.y < 0i) || (_e22.y >= _e24.y)) {
        {
            return vec3(0f);
        }
    }
    let _e31: vec2<i32> = p_1;
    if (_e31.x >= 0i) {
        let _e35: vec2<i32> = p_1;
        let _e37: vec2<i32> = size;
        local = (_e35.x % _e37.x);
    } else {
        let _e40: vec2<i32> = size;
        let _e46: vec2<i32> = p_1;
        let _e49: vec2<i32> = size;
        local = ((_e40.x - 1i) - ((-1i - _e46.x) % _e49.x));
    }
    let _e54: i32 = local;
    p_1.x = _e54;
    let _e55: vec2<i32> = p_1;
    let _e57: vec4<f32> = textureLoad(wgpu_src, _e55, 0i);
    return _e57.xyz;
}

fn tap(uv: vec2<f32>) -> vec3<f32> {
    var uv_1: vec2<f32>;
    var p_2: vec2<f32>;
    var lo: vec2<i32>;
    var f: vec2<f32>;

    uv_1 = uv;
    let _e14: vec2<f32> = uv_1;
    let _e16: vec2<u32> = textureDimensions(wgpu_src, 0i);
    p_2 = ((_e14 * vec2<f32>(vec2<i32>(_e16))) - vec2(0.5f));
    let _e24: vec2<f32> = p_2;
    lo = vec2<i32>(floor(_e24));
    let _e28: vec2<f32> = p_2;
    f = fract(_e28);
    let _e31: vec2<i32> = lo;
    let _e32: vec3<f32> = borderTexel(_e31);
    let _e33: vec2<i32> = lo;
    let _e38: vec3<f32> = borderTexel((_e33 + vec2<i32>(1i, 0i)));
    let _e39: vec2<f32> = f;
    let _e43: vec2<i32> = lo;
    let _e48: vec3<f32> = borderTexel((_e43 + vec2<i32>(0i, 1i)));
    let _e49: vec2<i32> = lo;
    let _e54: vec3<f32> = borderTexel((_e49 + vec2<i32>(1i, 1i)));
    let _e55: vec2<f32> = f;
    let _e59: vec2<f32> = f;
    return mix(mix(_e32, _e38, vec3(_e39.x)), mix(_e48, _e54, vec3(_e55.x)), vec3(_e59.y));
}

fn main_1() {
    var mode: i32;
    var srcSize: vec2<f32>;
    var dstSize: vec2<f32>;
    var uv_2: vec2<f32>;
    var t: vec2<f32>;
    var s: vec3<f32>;
    var p_3: vec2<i32>;
    var size_1: vec2<i32>;
    var local_1: vec2<i32>;
    var dir: vec2<i32>;
    var sigma: f32;
    var n: i32;
    var s_1: vec3<f32> = vec3(0f);
    var wsum: f32 = 0f;
    var i: i32;
    var w: f32;
    var q: vec2<i32>;
    var local_2: i32;
    var h: vec2<f32>;
    var s_2: vec3<f32>;

    let _e12: Bloom = pc;
    mode = _e12.a.x;
    let _e16: Bloom = pc;
    srcSize = vec2<f32>(_e16.a.yz);
    let _e21: Bloom = pc;
    dstSize = vec2<f32>(_e21.b.xy);
    let _e27: vec4<f32> = gl_FragCoord_1;
    let _e29: vec2<f32> = dstSize;
    uv_2 = (_e27.xy / _e29);
    let _e33: vec2<f32> = srcSize;
    t = (vec2(1f) / _e33);
    let _e37: i32 = mode;
    if (_e37 == 0i) {
        {
            let _e40: vec2<f32> = uv_2;
            let _e41: vec3<f32> = tap(_e40);
            s = (_e41 * 4f);
            let _e45: vec3<f32> = s;
            let _e46: vec2<f32> = uv_2;
            let _e47: vec2<f32> = t;
            let _e49: vec3<f32> = tap((_e46 - _e47));
            s = (_e45 + _e49);
            let _e51: vec3<f32> = s;
            let _e52: vec2<f32> = uv_2;
            let _e53: vec2<f32> = t;
            let _e55: vec3<f32> = tap((_e52 + _e53));
            s = (_e51 + _e55);
            let _e57: vec3<f32> = s;
            let _e58: vec2<f32> = uv_2;
            let _e59: vec2<f32> = t;
            let _e61: vec2<f32> = t;
            let _e66: vec3<f32> = tap((_e58 + vec2<f32>(_e59.x, -(_e61.y))));
            s = (_e57 + _e66);
            let _e68: vec3<f32> = s;
            let _e69: vec2<f32> = uv_2;
            let _e70: vec2<f32> = t;
            let _e73: vec2<f32> = t;
            let _e77: vec3<f32> = tap((_e69 + vec2<f32>(-(_e70.x), _e73.y)));
            s = (_e68 + _e77);
            let _e79: vec3<f32> = s;
            let _e82: vec3<f32> = (_e79 / vec3(8f));
            outColour = vec4<f32>(_e82.x, _e82.y, _e82.z, 1f);
            return;
        }
    }
    let _e88: i32 = mode;
    let _e91: i32 = mode;
    if ((_e88 == 1i) || (_e91 == 2i)) {
        {
            let _e95: vec4<f32> = gl_FragCoord_1;
            p_3 = vec2<i32>(_e95.xy);
            let _e99: Bloom = pc;
            size_1 = vec2<i32>(_e99.a.yz);
            let _e104: i32 = mode;
            if (_e104 == 1i) {
                local_1 = vec2<i32>(1i, 0i);
            } else {
                local_1 = vec2<i32>(0i, 1i);
            }
            let _e114: vec2<i32> = local_1;
            dir = _e114;
            let _e116: Bloom = pc;
            sigma = max(_e116.k.y, 0.001f);
            let _e122: Bloom = pc;
            n = _e122.a.w;
            let _e131: i32 = n;
            i = -(_e131);
            loop {
                let _e134: i32 = i;
                let _e135: i32 = n;
                if !((_e134 <= _e135)) {
                    break;
                }
                {
                    let _e141: i32 = i;
                    let _e142: i32 = i;
                    let _e147: f32 = sigma;
                    let _e149: f32 = sigma;
                    w = exp((-(f32((_e141 * _e142))) / ((2f * _e147) * _e149)));
                    let _e154: f32 = wsum;
                    let _e155: f32 = w;
                    wsum = (_e154 + _e155);
                    let _e157: vec2<i32> = p_3;
                    let _e158: vec2<i32> = dir;
                    let _e159: i32 = i;
                    q = (_e157 + (_e158 * _e159));
                    let _e163: vec2<i32> = q;
                    let _e167: vec2<i32> = q;
                    let _e169: vec2<i32> = size_1;
                    if ((_e163.y < 0i) || (_e167.y >= _e169.y)) {
                        {
                            continue;
                        }
                    }
                    let _e174: vec2<i32> = q;
                    if (_e174.x >= 0i) {
                        let _e178: vec2<i32> = q;
                        let _e180: vec2<i32> = size_1;
                        local_2 = (_e178.x % _e180.x);
                    } else {
                        let _e183: vec2<i32> = size_1;
                        let _e189: vec2<i32> = q;
                        let _e192: vec2<i32> = size_1;
                        local_2 = ((_e183.x - 1i) - ((-1i - _e189.x) % _e192.x));
                    }
                    let _e197: i32 = local_2;
                    q.x = _e197;
                    let _e198: vec3<f32> = s_1;
                    let _e199: vec2<i32> = q;
                    let _e201: vec4<f32> = textureLoad(wgpu_src, _e199, 0i);
                    let _e203: f32 = w;
                    s_1 = (_e198 + (_e201.xyz * _e203));
                }
                continuing {
                    let _e138: i32 = i;
                    i = (_e138 + 1i);
                }
            }
            let _e206: vec3<f32> = s_1;
            let _e207: f32 = wsum;
            let _e209: vec3<f32> = (_e206 / vec3(_e207));
            outColour = vec4<f32>(_e209.x, _e209.y, _e209.z, 1f);
            return;
        }
    }
    let _e215: i32 = mode;
    if (_e215 == 3i) {
        {
            let _e219: vec2<f32> = t;
            h = (0.5f * _e219);
            let _e222: vec2<f32> = uv_2;
            let _e225: vec2<f32> = h;
            let _e231: vec3<f32> = tap((_e222 + vec2<f32>((-2f * _e225.x), 0f)));
            s_2 = _e231;
            let _e233: vec3<f32> = s_2;
            let _e234: vec2<f32> = uv_2;
            let _e235: vec2<f32> = h;
            let _e238: vec2<f32> = h;
            let _e242: vec3<f32> = tap((_e234 + vec2<f32>(-(_e235.x), _e238.y)));
            s_2 = (_e233 + (_e242 * 2f));
            let _e246: vec3<f32> = s_2;
            let _e247: vec2<f32> = uv_2;
            let _e250: vec2<f32> = h;
            let _e255: vec3<f32> = tap((_e247 + vec2<f32>(0f, (2f * _e250.y))));
            s_2 = (_e246 + _e255);
            let _e257: vec3<f32> = s_2;
            let _e258: vec2<f32> = uv_2;
            let _e259: vec2<f32> = h;
            let _e261: vec2<f32> = h;
            let _e265: vec3<f32> = tap((_e258 + vec2<f32>(_e259.x, _e261.y)));
            s_2 = (_e257 + (_e265 * 2f));
            let _e269: vec3<f32> = s_2;
            let _e270: vec2<f32> = uv_2;
            let _e272: vec2<f32> = h;
            let _e278: vec3<f32> = tap((_e270 + vec2<f32>((2f * _e272.x), 0f)));
            s_2 = (_e269 + _e278);
            let _e280: vec3<f32> = s_2;
            let _e281: vec2<f32> = uv_2;
            let _e282: vec2<f32> = h;
            let _e284: vec2<f32> = h;
            let _e289: vec3<f32> = tap((_e281 + vec2<f32>(_e282.x, -(_e284.y))));
            s_2 = (_e280 + (_e289 * 2f));
            let _e293: vec3<f32> = s_2;
            let _e294: vec2<f32> = uv_2;
            let _e298: vec2<f32> = h;
            let _e303: vec3<f32> = tap((_e294 + vec2<f32>(0f, (-2f * _e298.y))));
            s_2 = (_e293 + _e303);
            let _e305: vec3<f32> = s_2;
            let _e306: vec2<f32> = uv_2;
            let _e307: vec2<f32> = h;
            let _e310: vec2<f32> = h;
            let _e315: vec3<f32> = tap((_e306 + vec2<f32>(-(_e307.x), -(_e310.y))));
            s_2 = (_e305 + (_e315 * 2f));
            let _e319: vec3<f32> = s_2;
            let _e322: vec3<f32> = (_e319 / vec3(12f));
            outColour = vec4<f32>(_e322.x, _e322.y, _e322.z, 1f);
            return;
        }
    }
    let _e328: vec4<f32> = gl_FragCoord_1;
    let _e332: vec4<f32> = textureLoad(wgpu_emitTex, vec2<i32>(_e328.xy), 0i);
    if any((_e332.xyz > vec3(0f))) {
        {
            outColour = vec4(0f);
            return;
        }
    }
    let _e340: vec2<f32> = uv_2;
    let _e341: vec3<f32> = tap(_e340);
    let _e342: Bloom = pc;
    let _e345: vec3<f32> = (_e341 * _e342.k.x);
    outColour = vec4<f32>(_e345.x, _e345.y, _e345.z, 0f);
    return;
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e20: vec4<f32> = outColour;
    return FragmentOutput(_e20);
}
