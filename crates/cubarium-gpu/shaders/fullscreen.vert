#version 450
// One oversized triangle. No vertex buffer, no index buffer: three vertices from
// `gl_VertexIndex` and the rasterizer clips the rest.
void main() {
    vec2 p = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
