#version 450
layout(location = 0) out vec2 v_uv;
void main() {
    vec2 uv = vec2(float(gl_VertexIndex & 1), float((gl_VertexIndex >> 1) & 1));
    v_uv = uv;
    gl_Position = vec4(uv * 1.6 - 0.8, 0.0, 1.0);
}
