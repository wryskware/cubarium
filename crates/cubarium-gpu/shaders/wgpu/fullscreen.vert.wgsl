struct VertexOutput {
    @builtin(position) gl_Position: vec4<f32>,
}

var<private> gl_VertexIndex_1: u32;
var<private> gl_Position: vec4<f32>;

fn main_1() {
    var p: vec2<f32>;

    let _e1: u32 = gl_VertexIndex_1;
    let _e8: u32 = gl_VertexIndex_1;
    p = vec2<f32>(f32(((i32(_e1) << 1u) & 2i)), f32((i32(_e8) & 2i)));
    let _e17: vec2<f32> = p;
    let _e22: vec2<f32> = ((_e17 * 2f) - vec2(1f));
    gl_Position = vec4<f32>(_e22.x, _e22.y, 0f, 1f);
    let _e29: vec4<f32> = gl_Position;
    gl_Position.y = -(_e29.y);
    return;
}

@vertex 
fn main(@builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e3: vec4<f32> = gl_Position;
    return VertexOutput(_e3);
}
