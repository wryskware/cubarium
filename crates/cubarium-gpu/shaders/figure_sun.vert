#version 450
// The figures' sun map (H2): every figure cell drawn along the sun, orthographic, keeping
// each sun ray's highest figure point (depth GREATER). A texel is where the ray meets
// y = 0: x − y·Lx/Ly round the ring, z − y·Lz/Ly from the map's first z.
//
// Push: k = (tile, W, H, map height), atlas = (Lx/Ly, Lz/Ly, first z, texels a voxel of z);
// two texels a voxel across.

#include "figure.glsl"

const vec3 FACE_O[6] = vec3[](vec3(0, 0, 1), vec3(1, 0, 0), vec3(0, 0, 1), vec3(0, 1, 0), vec3(0, 0, 0), vec3(1, 0, 1));
const vec3 FACE_U[6] = vec3[](vec3(0, 0, -1), vec3(0, 0, 1), vec3(1, 0, 0), vec3(1, 0, 0), vec3(1, 0, 0), vec3(-1, 0, 0));
const vec3 FACE_V[6] = vec3[](vec3(0, 1, 0), vec3(0, 1, 0), vec3(0, 0, -1), vec3(0, 0, 1), vec3(0, 1, 0), vec3(0, 1, 0));
const vec2 CORNER[6] = vec2[](vec2(0, 0), vec2(1, 0), vec2(1, 1), vec2(0, 0), vec2(1, 1), vec2(0, 1));

int sbyte(uint v, int at) { return int(bitfieldExtract(int(v), at, 8)); }

void main() {
    Tile t = tiles[pc.k.x];
    uvec2 cell = vox[gl_InstanceIndex];
    int face = gl_VertexIndex / 6;
    vec2 ab = CORNER[gl_VertexIndex % 6];
    int cls = int(cell.y & 7u);
    bool head = (cell.y & 8u) != 0u;
    vec3 c = vec3(float(sbyte(cell.x, 0)), float(sbyte(cell.x, 8)), float(sbyte(cell.x, 16)));
    float settle = head ? t.az.w : t.ax.w;
    float breath = head ? t.depth.z : t.ay.w;
    float y0 = c.y;
    float y1 = c.y + 1.0;
    if (cls == 1 || cls == 2) {
        y0 -= settle;
        y1 -= settle + (cls == 2 ? breath : 0.0);
    } else if (cls == 3 || cls == 4) {
        y1 = max(y0, min(y1, c.y + float(cls - 2) - settle));
    }
    vec3 unit = FACE_O[face] + ab.x * FACE_U[face] + ab.y * FACE_V[face];
    vec3 local = vec3(c.x - 0.5 + unit.x, mix(y0, y1, unit.y), c.z - 0.5 + unit.z);
    mat3 m = mat3(t.ax.xyz, t.ay.xyz, t.az.xyz);
    vec3 q = t.pos.xyz + m * local;
    vec3 mid = t.pos.xyz + m * vec3(c.x, 0.5 * (y0 + y1), c.z);
    float w = float(pc.k.y);
    // Round the ring by the cell's middle, so a cell is never torn across the seam.
    float shift = w * floor((mid.x - mid.y * pc.atlas.x) / w);
    float fx = q.x - q.y * pc.atlas.x - shift;
    float fz = q.z - q.y * pc.atlas.y - pc.atlas.z;
    // Two texels a voxel across a map 2W wide: fx / W of its width.
    vec2 map = vec2(fx / w, fz * pc.atlas.w / float(pc.k.w));
    gl_Position = vec4(map * 2.0 - 1.0, clamp(q.y / float(pc.k.z), 0.0, 1.0), 1.0);
}
