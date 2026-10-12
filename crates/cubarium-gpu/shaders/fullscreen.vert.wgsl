// One oversized triangle. No vertex buffer, no index buffer: three vertices from
// the vertex index and the rasterizer clips the rest.
@vertex
fn main(@builtin(vertex_index) vertexIndex: u32) -> @builtin(position) vec4<f32> {
    let i = i32(vertexIndex);
    let p = vec2<f32>(f32((i << 1u) & 2), f32(i & 2));
    let position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    // Written for Vulkan's downward clip-space y; WebGPU's points up.
    return vec4<f32>(position.x, -position.y, position.zw);
}
