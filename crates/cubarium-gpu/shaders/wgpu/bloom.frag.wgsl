struct Bloom {
    a: vec4<i32>,
    b: vec4<i32>,
    k: vec4<f32>,
}

struct FragmentOutput {
    @location(0) outColour: vec4<f32>,
}

const STEPS: f32 = 3f;

@group(0) @binding(32) 
var wgpu_sampler_0_32_: sampler;
var<immediate> pc: Bloom;
@group(0) @binding(0) 
var wgpu_src: texture_2d<f32>;
@group(0) @binding(1) 
var wgpu_emitTex: texture_2d<f32>;
var<private> outColour: vec4<f32>;
var<private> gl_FragCoord_1: vec4<f32>;

fn main_1() {
    var mode: i32;
    var S: i32;
    var R: i32;
    var CW: i32;
    var CH: i32;
    var OFF: i32;
    var RH: i32;
    var p: vec2<i32>;
    var e: vec3<f32> = vec3(0f);
    var y0_: i32;
    var j: i32 = 0i;
    var y: i32;
    var i: i32;
    var best: vec3<f32> = vec3(0f);
    var reach: f32;
    var dy: i32;
    var y_1: i32;
    var dx: i32;
    var d: f32;
    var x: i32;
    var local: i32;
    var e_1: vec3<f32>;
    var w: f32;
    var q: f32;
    var c: vec2<i32>;
    var h: vec3<f32>;

    let _e12: Bloom = pc;
    mode = _e12.a.x;
    let _e16: Bloom = pc;
    S = _e16.a.y;
    let _e20: Bloom = pc;
    R = _e20.a.z;
    let _e24: Bloom = pc;
    CW = _e24.b.x;
    let _e28: Bloom = pc;
    CH = _e28.b.y;
    let _e32: Bloom = pc;
    OFF = _e32.b.z;
    let _e36: Bloom = pc;
    RH = _e36.b.w;
    let _e41: vec4<f32> = gl_FragCoord_1;
    p = vec2<i32>(_e41.xy);
    let _e45: i32 = mode;
    if (_e45 == 0i) {
        {
            let _e51: vec2<i32> = p;
            let _e53: i32 = S;
            let _e55: i32 = OFF;
            y0_ = ((_e51.y * _e53) - _e55);
            loop {
                let _e60: i32 = j;
                let _e61: i32 = S;
                if !((_e60 < _e61)) {
                    break;
                }
                {
                    let _e67: i32 = y0_;
                    let _e68: i32 = j;
                    y = (_e67 + _e68);
                    let _e71: i32 = y;
                    let _e74: i32 = y;
                    let _e75: i32 = RH;
                    if ((_e71 < 0i) || (_e74 >= _e75)) {
                        {
                            continue;
                        }
                    }
                    i = 0i;
                    loop {
                        let _e80: i32 = i;
                        let _e81: i32 = S;
                        if !((_e80 < _e81)) {
                            break;
                        }
                        {
                            let _e87: vec3<f32> = e;
                            let _e88: vec2<i32> = p;
                            let _e90: i32 = S;
                            let _e92: i32 = i;
                            let _e94: i32 = y;
                            let _e97: vec4<f32> = textureLoad(wgpu_src, vec2<i32>(((_e88.x * _e90) + _e92), _e94), 0i);
                            e = max(_e87, _e97.xyz);
                        }
                        continuing {
                            let _e84: i32 = i;
                            i = (_e84 + 1i);
                        }
                    }
                }
                continuing {
                    let _e64: i32 = j;
                    j = (_e64 + 1i);
                }
            }
            let _e100: vec3<f32> = e;
            outColour = vec4<f32>(_e100.x, _e100.y, _e100.z, 1f);
            return;
        }
    }
    let _e106: i32 = mode;
    if (_e106 == 1i) {
        {
            let _e112: i32 = R;
            reach = (f32(_e112) + 0.5f);
            let _e117: i32 = R;
            dy = -(_e117);
            loop {
                let _e120: i32 = dy;
                let _e121: i32 = R;
                if !((_e120 <= _e121)) {
                    break;
                }
                {
                    let _e127: vec2<i32> = p;
                    let _e129: i32 = dy;
                    y_1 = (_e127.y + _e129);
                    let _e132: i32 = y_1;
                    let _e135: i32 = y_1;
                    let _e136: i32 = CH;
                    if ((_e132 < 0i) || (_e135 >= _e136)) {
                        {
                            continue;
                        }
                    }
                    let _e139: i32 = R;
                    dx = -(_e139);
                    loop {
                        let _e142: i32 = dx;
                        let _e143: i32 = R;
                        if !((_e142 <= _e143)) {
                            break;
                        }
                        {
                            let _e149: i32 = dx;
                            let _e150: i32 = dy;
                            d = length(vec2<f32>(f32(_e149), f32(_e150)));
                            let _e156: f32 = d;
                            let _e157: f32 = reach;
                            if (_e156 > _e157) {
                                {
                                    continue;
                                }
                            }
                            let _e159: vec2<i32> = p;
                            let _e161: i32 = dx;
                            x = (_e159.x + _e161);
                            let _e164: i32 = x;
                            if (_e164 >= 0i) {
                                let _e167: i32 = x;
                                let _e168: i32 = CW;
                                local = (_e167 % _e168);
                            } else {
                                let _e170: i32 = CW;
                                let _e175: i32 = x;
                                let _e177: i32 = CW;
                                local = ((_e170 - 1i) - ((-1i - _e175) % _e177));
                            }
                            let _e181: i32 = local;
                            x = _e181;
                            let _e182: i32 = x;
                            let _e183: i32 = y_1;
                            let _e186: vec4<f32> = textureLoad(wgpu_src, vec2<i32>(_e182, _e183), 0i);
                            e_1 = _e186.xyz;
                            let _e190: f32 = d;
                            let _e191: i32 = R;
                            w = (1f - (_e190 / f32((_e191 + 1i))));
                            let _e198: f32 = w;
                            q = (ceil(((_e198 * STEPS) - 0.001f)) / STEPS);
                            let _e205: vec3<f32> = best;
                            let _e206: vec3<f32> = e_1;
                            let _e207: f32 = q;
                            best = max(_e205, (_e206 * _e207));
                        }
                        continuing {
                            let _e146: i32 = dx;
                            dx = (_e146 + 1i);
                        }
                    }
                }
                continuing {
                    let _e124: i32 = dy;
                    dy = (_e124 + 1i);
                }
            }
            let _e210: vec3<f32> = best;
            outColour = vec4<f32>(_e210.x, _e210.y, _e210.z, 1f);
            return;
        }
    }
    let _e216: vec2<i32> = p;
    let _e218: vec4<f32> = textureLoad(wgpu_emitTex, _e216, 0i);
    if any((_e218.xyz > vec3(0f))) {
        {
            outColour = vec4(0f);
            return;
        }
    }
    let _e226: vec2<i32> = p;
    let _e228: i32 = S;
    let _e230: vec2<i32> = p;
    let _e232: i32 = OFF;
    let _e234: i32 = S;
    c = vec2<i32>((_e226.x / _e228), ((_e230.y + _e232) / _e234));
    let _e238: vec2<i32> = c;
    let _e240: vec4<f32> = textureLoad(wgpu_src, _e238, 0i);
    h = _e240.xyz;
    let _e243: vec3<f32> = h;
    let _e244: Bloom = pc;
    let _e247: vec3<f32> = (_e243 * _e244.k.x);
    outColour = vec4<f32>(_e247.x, _e247.y, _e247.z, 0f);
    return;
}

@fragment 
fn main(@builtin(position) gl_FragCoord: vec4<f32>) -> FragmentOutput {
    gl_FragCoord_1 = gl_FragCoord;
    main_1();
    let _e20: vec4<f32> = outColour;
    return FragmentOutput(_e20);
}
