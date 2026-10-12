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
// what the stamp drew. The sleep and idle poses move it as the walk's `posedSpan` did:
// a torso cell down by the settle, a breathing cell's top by the breath, a leg cell's top
// cut where the settling torso's bottom has come down to. A rigged figure's cell then moves
// by its bone (FEED-RIG, `bonePose`), the cube turned with it, its normal with it.

// #import figure.wgsl

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) vWorld: vec3<f32>,
    @location(1) vNormal: vec3<f32>,
    // Across the face, 0..1, and down it from its top edge (a side), or from its far edge (a
    // top or a bottom).
    @location(2) vFace: vec2<f32>,
    // The palette entry, glyph (0: baked model), its top's lip open (bit 20), local face + crease mask.
    @location(3) @interpolate(flat) vInfo: vec4<i32>,
    @location(4) @interpolate(flat) vFaceSize: vec2<i32>,
    @location(5) vRimWorld: vec3<f32>,
    @location(6) vRimFace: vec2<f32>,
    @location(7) @interpolate(flat) vCapSize: vec2<i32>,
}

fn faceSize(t: Tile, face: i32) -> vec2<i32> {
    if (face < 2) { return t.faceSize.zy; }
    if (face < 4) { return t.faceSize.xz; }
    return t.faceSize.xy;
}

@vertex
fn main(@builtin(instance_index) instanceIndex: u32, @builtin(vertex_index) vertexIndex: u32) -> Varyings {
    let vertex = i32(vertexIndex);
    let t = tiles[pc.k.x];
    let cell = vox[i32(instanceIndex)];
    let face = vertex / 6;
    let ab = CORNER[vertex % 6];
    let cls = i32(cell.y & 7u);
    let head = (cell.y & 8u) != 0u;
    let open = (cell.y >> 4u) & 63u;
    let glyph = i32((cell.y >> 10u) & 15u);
    let row = boneRow(t, cell.y);
    var nLocal = FACE_N[face];
    if (row >= 0) { nLocal = boneTurn(row, FACE_N[face]); }
    let n = mat3x3(t.ax.xyz, t.ay.xyz, t.az.xyz) * nLocal;
    var out: Varyings;
    // A face against another cell of the model, or turned away from the camera (the view
    // runs along (0, -rise/s, 1)), is never seen: off the clip volume.
    let seen = (open & (1u << u32(face))) != 0u && n.z * f32(pc.k.y) - n.y * f32(pc.k.z) < 1e-4;
    if (!seen) {
        out.position = vec4(2.0, -2.0, 2.0, 1.0);
        out.vWorld = vec3(0.0);
        out.vNormal = vec3(0.0, 1.0, 0.0);
        out.vFace = vec2(0.0);
        out.vInfo = vec4(0);
        out.vFaceSize = vec2(1);
        out.vCapSize = vec2(1);
        out.vRimWorld = vec3(0.0);
        out.vRimFace = vec2(0.0);
        return out;
    }
    let c = vec3(f32(sbyte(cell.x, 0u)), f32(sbyte(cell.x, 8u)), f32(sbyte(cell.x, 16u)));
    // The pose: the body's settle and breath, or the head's while it moves.
    let settle = select(t.ax.w, t.az.w, head);
    let breath = select(t.ay.w, t.depth.z, head);
    var y0 = c.y;
    var y1 = c.y + 1.0;
    if (cls == 1 || cls == 2) {
        y0 -= settle;
        y1 -= settle + select(0.0, breath, cls == 2);
    } else if (cls == 3 || cls == 4) {
        let d = f32(cls - 2);
        y1 = max(y0, min(y1, c.y + d - settle));
    }
    let unit = FACE_O[face] + ab.x * FACE_U[face] + ab.y * FACE_V[face];
    let local = vec3(c.x - 0.5 + unit.x, mix(y0, y1, unit.y), c.z - 0.5 + unit.z);
    let posed = bonePose(row, local);
    let rel = t.ax.xyz * posed.x + t.ay.xyz * posed.y + t.az.xyz * posed.z;
    let q = t.pos.xyz + rel;
    // Screen pixels from P's projection, then the tile's texels, then the atlas.
    var screen = vec2(rel.x * f32(pc.k.y), -(rel.y * f32(pc.k.y) + rel.z * f32(pc.k.z)));
    // Half-voxel bounds otherwise sit on pixel centres at odd scales. Use the
    // stamp's sampling phase; pack adds it to the composite anchor before snap.
    screen -= vec2<f32>(vec2(pc.k.y & 1, pc.k.z & 1)) * 0.5;
    // Keep the origin and face tones in output pixels. Only the raster lattice grows:
    // a one-pixel outline therefore spans aa samples rather than becoming thinner.
    var atlas: vec2<f32>;
    if (t.faceSize.w == 1) {
        atlas = vec2<f32>(t.rect.xy) + screen - vec2<f32>(t.orig.xy);
    } else {
        atlas = vec2<f32>(t.rect.xy) + (screen - vec2<f32>(t.orig.xy)) * f32(t.faceSize.w);
    }
    let depth = clamp(0.5 + rel.z * t.depth.y, 0.0, 1.0);
    let position = vec4(atlas / pc.atlas.xy * 2.0 - 1.0, depth, 1.0);
    // Written for Vulkan's downward clip-space y; WebGPU's points up.
    out.position = vec4(position.x, -position.y, position.zw);
    out.vWorld = q;
    out.vFaceSize = faceSize(t, face);
    out.vCapSize = faceSize(t, 3);
    // The nearest cap texel to this side, as shadeTop uses for the stamp's rim.
    let inward = 0.5 / f32(min(out.vCapSize.x, out.vCapSize.y));
    let rim = unit - FACE_N[face] * inward;
    let rimLocal = bonePose(row, vec3(local.x, y1, local.z) - FACE_N[face] * inward);
    out.vRimWorld = t.pos.xyz + mat3x3(t.ax.xyz, t.ay.xyz, t.az.xyz) * rimLocal;
    out.vRimFace = vec2(rim.x, 1.0 - rim.z);
    out.vNormal = n;
    out.vFace = vec2(ab.x, 1.0 - ab.y);
    out.vInfo = vec4(t.orig.z + i32(cell.x >> 24u), glyph, i32((cell.y >> 20u) & 1u),
        face | i32(((cell[2 + face / 4] >> (8u * u32(face % 4))) & 255u) << 3u) | i32((cell.z >> 24u) << 11u));
    return out;
}
