// The figures' sun map (H2): every figure cell drawn along the sun, orthographic, keeping
// each sun ray's highest figure point (depth GREATER). A texel is where the ray meets
// y = 0: x − y·Lx/Ly round the ring, z − y·Lz/Ly from the map's first z.
//
// Push: k = (tile, W, H, map height), atlas = (Lx/Ly, Lz/Ly, first z, texels a voxel of z);
// two texels a voxel across.

// #import figure.wgsl

@vertex
fn main(@builtin(instance_index) instanceIndex: u32, @builtin(vertex_index) vertexIndex: u32) -> @builtin(position) vec4<f32> {
    let vertex = i32(vertexIndex);
    let t = tiles[pc.k.x];
    let cell = vox[i32(instanceIndex)];
    let face = vertex / 6;
    let ab = CORNER[vertex % 6];
    let cls = i32(cell.y & 7u);
    let head = (cell.y & 8u) != 0u;
    let c = vec3(f32(sbyte(cell.x, 0u)), f32(sbyte(cell.x, 8u)), f32(sbyte(cell.x, 16u)));
    let settle = select(t.ax.w, t.az.w, head);
    let breath = select(t.ay.w, t.depth.z, head);
    var y0 = c.y;
    var y1 = c.y + 1.0;
    if (cls == 1 || cls == 2) {
        y0 -= settle;
        y1 -= settle + select(0.0, breath, cls == 2);
    } else if (cls == 3 || cls == 4) {
        y1 = max(y0, min(y1, c.y + f32(cls - 2) - settle));
    }
    let unit = FACE_O[face] + ab.x * FACE_U[face] + ab.y * FACE_V[face];
    let local = vec3(c.x - 0.5 + unit.x, mix(y0, y1, unit.y), c.z - 0.5 + unit.z);
    let m = mat3x3(t.ax.xyz, t.ay.xyz, t.az.xyz);
    // A rigged cell moves by its bone as `figure.vert.wgsl` moves it (FEED-RIG).
    let row = boneRow(t, cell.y);
    let centre = vec3(c.x, 0.5 * (y0 + y1), c.z);
    let q = t.pos.xyz + m * bonePose(row, local);
    let mid = t.pos.xyz + m * bonePose(row, centre);
    let w = f32(pc.k.y);
    // Round the ring by the cell's middle, so a cell is never torn across the seam.
    let shift = w * floor((mid.x - mid.y * pc.atlas.x) / w);
    let fx = q.x - q.y * pc.atlas.x - shift;
    let fz = q.z - q.y * pc.atlas.y - pc.atlas.z;
    // Two texels a voxel across a map 2W wide: fx / W of its width.
    let map = vec2(fx / w, fz * pc.atlas.w / f32(pc.k.w));
    let position = vec4(map * 2.0 - 1.0, clamp(q.y / f32(pc.k.z), 0.0, 1.0), 1.0);
    // Written for Vulkan's downward clip-space y; WebGPU's points up.
    return vec4(position.x, -position.y, position.zw);
}
