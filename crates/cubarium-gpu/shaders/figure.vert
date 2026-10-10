#version 450
// The model pass's tile raster (H2 of `design/handoffs/hybrid-renderer-2026-10-07.md`): one
// instance a model voxel, 36 vertices a cube, into one figure's tile of the atlas.
//
// The tile's pixel grid is the raster's own (`s` pixels a voxel, `rise` a voxel of depth)
// anchored at P, the figure's place in the world: a world point q lands on tile texel
// (q - P) projected, less the tile's origin. So the tile holds the same pixels wherever the
// figure stands, and the composite pass places it at P's exact projection: the figure moves
// with no crawling pixels, and only a turn or a pose changes which pixels it covers.
// At odd S/RISE, an equal and opposite half-texel phase in tile and composite keeps
// quarter-heading face samples centred on the stamped world's texels.
//
// A cell (dx, dy, dz) of the model spans x in dx -+ 1/2, y in dy .. dy + 1, z in dz -+ 1/2
// of the model's own frame (forward +x, up +y, side +z), turned about its anchor column as
// the stamped models turn (`cubarium::voxel::model::turn`), so a quarter heading draws
// what the stamp drew. The sleep and idle poses move it as `voxel.frag`'s `posedSpan` did:
// a torso cell down by the settle, a breathing cell's top by the breath, a leg cell's top
// cut where the settling torso's bottom has come down to. A rigged figure's cell then moves
// by its bone (FEED-RIG, `bonePose`), the cube turned with it, its normal with it.

#include "figure.glsl"

layout(location = 0) out vec3 vWorld;
layout(location = 1) out vec3 vNormal;
// Across the face, 0..1, and down it from its top edge (a side), or from its far edge (a
// top or a bottom).
layout(location = 2) out vec2 vFace;
// The palette entry, glyph (0: baked model), its top's lip open (bit 20), local face + crease mask.
layout(location = 3) flat out ivec4 vInfo;
layout(location = 4) flat out ivec2 vFaceSize;
layout(location = 5) out vec3 vRimWorld;
layout(location = 6) out vec2 vRimFace;
layout(location = 7) flat out ivec2 vCapSize;

ivec2 faceSize(Tile t, int face) {
    if (face < 2) { return t.faceSize.zy; }
    if (face < 4) { return t.faceSize.xz; }
    return t.faceSize.xy;
}

// Each face: a corner, the axis across it and the axis up it, in the cell's unit box.
// Faces: -x, +x, -y, +y, -z, +z (the open mask's bits, in this order).
// Two triangles of the unit square.
const vec2 CORNER[6] = vec2[](vec2(0, 0), vec2(1, 0), vec2(1, 1), vec2(0, 0), vec2(1, 1), vec2(0, 1));

int sbyte(uint v, int at) { return int(bitfieldExtract(int(v), at, 8)); }

void main() {
    Tile t = tiles[pc.k.x];
    uvec4 cell = vox[gl_InstanceIndex];
    int face = gl_VertexIndex / 6;
    vec2 ab = CORNER[gl_VertexIndex % 6];
    int cls = int(cell.y & 7u);
    bool head = (cell.y & 8u) != 0u;
    uint open = (cell.y >> 4) & 63u;
    int glyph = int((cell.y >> 10) & 15u);
    int row = boneRow(t, cell.y);
    vec3 nLocal = row >= 0 ? boneTurn(row, FACE_N[face]) : FACE_N[face];
    vec3 n = mat3(t.ax.xyz, t.ay.xyz, t.az.xyz) * nLocal;
    // A face against another cell of the model, or turned away from the camera (the view
    // runs along (0, -rise/s, 1)), is never seen: off the clip volume.
    bool seen = (open & (1u << uint(face))) != 0u && n.z * float(pc.k.y) - n.y * float(pc.k.z) < 1e-4;
    if (!seen) {
        gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
        vWorld = vec3(0.0);
        vNormal = vec3(0.0, 1.0, 0.0);
        vFace = vec2(0.0);
        vInfo = ivec4(0);
        vFaceSize = vCapSize = ivec2(1);
        vRimWorld = vec3(0.0);
        vRimFace = vec2(0.0);
        return;
    }
    vec3 c = vec3(float(sbyte(cell.x, 0)), float(sbyte(cell.x, 8)), float(sbyte(cell.x, 16)));
    // The pose: the body's settle and breath, or the head's while it moves.
    float settle = head ? t.az.w : t.ax.w;
    float breath = head ? t.depth.z : t.ay.w;
    float y0 = c.y;
    float y1 = c.y + 1.0;
    if (cls == 1 || cls == 2) {
        y0 -= settle;
        y1 -= settle + (cls == 2 ? breath : 0.0);
    } else if (cls == 3 || cls == 4) {
        float d = float(cls - 2);
        y1 = max(y0, min(y1, c.y + d - settle));
    }
    vec3 unit = FACE_O[face] + ab.x * FACE_U[face] + ab.y * FACE_V[face];
    vec3 local = vec3(c.x - 0.5 + unit.x, mix(y0, y1, unit.y), c.z - 0.5 + unit.z);
    vec3 posed = bonePose(row, local);
    vec3 rel = t.ax.xyz * posed.x + t.ay.xyz * posed.y + t.az.xyz * posed.z;
    vec3 q = t.pos.xyz + rel;
    // Screen pixels from P's projection, then the tile's texels, then the atlas.
    vec2 screen = vec2(rel.x * float(pc.k.y), -(rel.y * float(pc.k.y) + rel.z * float(pc.k.z)));
    // Half-voxel bounds otherwise sit on pixel centres at odd scales. Use the
    // stamp's sampling phase; pack adds it to the composite anchor before snap.
    screen -= vec2(pc.k.y & 1, pc.k.z & 1) * 0.5;
    // Keep the origin and face tones in output pixels. Only the raster lattice grows:
    // a one-pixel outline therefore spans aa samples rather than becoming thinner.
    vec2 atlas = t.faceSize.w == 1
        ? vec2(t.rect.xy) + screen - vec2(t.orig.xy)
        : vec2(t.rect.xy) + (screen - vec2(t.orig.xy)) * float(t.faceSize.w);
    float depth = clamp(0.5 + rel.z * t.depth.y, 0.0, 1.0);
    gl_Position = vec4(atlas / pc.atlas.xy * 2.0 - 1.0, depth, 1.0);
    vWorld = q;
    vFaceSize = faceSize(t, face);
    vCapSize = faceSize(t, 3);
    // The nearest cap texel to this side, as shadeTop uses for the stamp's rim.
    float inward = 0.5 / float(min(vCapSize.x, vCapSize.y));
    vec3 rim = unit - FACE_N[face] * inward;
    vec3 rimLocal = bonePose(row, vec3(local.x, y1, local.z) - FACE_N[face] * inward);
    vRimWorld = t.pos.xyz + mat3(t.ax.xyz, t.ay.xyz, t.az.xyz) * rimLocal;
    vRimFace = vec2(rim.x, 1.0 - rim.z);
    vNormal = n;
    vFace = vec2(ab.x, 1.0 - ab.y);
    vInfo = ivec4(t.orig.z + int(cell.x >> 24), glyph, int((cell.y >> 20) & 1u), face | int(((cell[2 + face / 4] >> (8 * (face % 4))) & 255u) << 3) | int((cell.z >> 24) << 11));
}
