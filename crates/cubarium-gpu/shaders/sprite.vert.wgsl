// #import scene.wgsl

// One instanced quad per stamp. The quad is built in the tile's *own* coordinates and
// rotated by the instance heading, so it is tight around the sprite rather than a
// conservative axis-aligned box. GS-1c narrowed it twice more: around the frames'
// **opaque** box instead of the whole tile, and clipped by the mask's own reveal.
//
// Nothing is interpolated. Every varying is flat, and the fragment recomputes its own
// tile coordinate from its position — see `sprite.frag.wgsl`.

struct Stamp {
    @location(0) iAnchor: vec2<f32>,
    @location(1) iHeading: vec2<f32>,
    @location(2) iFrames01: vec4<u32>,   // frame0.xy, frame1.xy
    @location(3) iFrames23: vec4<u32>,   // frame2.xy, frame3.xy
    @location(4) iSizePivot: vec4<u32>,  // size.xy, pivot.xy
    @location(5) iWeights: vec4<f32>,
    @location(6) iBend: vec4<f32>,       // amplitude, base, root, length
    @location(7) iMask: vec4<f32>,       // floor, reveal, flags, opacity
    @location(8) iTone: vec4<f32>,       // colour.rgb, mix
    @location(9) iShade: vec4<f32>,      // shade floor, reference, scale, source
    @location(10) iBox: vec4<f32>,       // opaque box in source texels: x0, y0, x1, y1
}

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) vPlace: vec4<f32>,      // the snapped anchor, then the heading
    @location(1) @interpolate(flat) vFrames01: vec4<u32>,
    @location(2) @interpolate(flat) vFrames23: vec4<u32>,
    @location(3) @interpolate(flat) vSizePivot: vec4<u32>,
    @location(4) @interpolate(flat) vWeights: vec4<f32>,
    @location(5) @interpolate(flat) vBend: vec4<f32>,
    @location(6) @interpolate(flat) vMask: vec4<f32>,
    @location(7) @interpolate(flat) vTone: vec4<f32>,
    @location(8) @interpolate(flat) vShade: vec4<f32>,
}

@vertex
fn main(s: Stamp, @builtin(vertex_index) vertexIndex: u32) -> Varyings {
    let vertex = i32(vertexIndex);
    let pivot = vec2<f32>(s.iSizePivot.zw);
    let tile = vec2<f32>(s.iSizePivot.xy);
    // The quad is built around the union of the live frames' **opaque** boxes, not
    // around the whole tile: 82 % of the pack's atlas texels are clear, and at
    // `--gpu-art-scale 2` on a 640x360 S=2 raster a whole 16x16 tile is a 64x64 quad.
    // A degenerate or missing box (a scratch frame, an instance nothing filled in)
    // falls back to the tile, which is the quad this shader always drew.
    var b0 = s.iBox.xy;
    var b1 = s.iBox.zw;
    if (!(b1.x > b0.x && b1.y > b0.y)) { b0 = vec2(0.0); b1 = tile; }

    // The mask cannot paint outside the rows it reveals, so it clips the box too.
    // `sprite.frag.wgsl`'s cover is `unit(reveal - h + 0.5) * unit(h - floor + 0.5)` with
    // `h = tile.y - src.y`, which is positive only for
    // `src.y` in `[tile.y - reveal - 0.5, tile.y - floor + 0.5]`. `NO_MASK_REVEAL` and
    // `NO_MASK_FLOOR` are +/-1e9, so an unmasked stamp's clamp is a no-op by arithmetic
    // rather than by a branch. A radial mask reveals a disc of `reveal + 0.5` about the
    // pivot instead, on both axes.
    if (s.iMask.z > 0.5) {
        let r = max(s.iMask.y, 0.0) + 0.5;
        b0 = max(b0, pivot - vec2(r));
        b1 = min(b1, pivot + vec2(r));
    } else {
        b0.y = max(b0.y, tile.y - s.iMask.y - 0.5);
        b1.y = min(b1.y, tile.y - s.iMask.x + 0.5);
    }

    // How far past the box a fragment can still read a texel inside it.
    //
    // Nearest (`u.time.z <= 0.5`): the fragment's own rule is `floor(src)`, so a
    // fragment paints iff `src` is *in* `[b0, b1)` — nothing outside the box can read
    // anything inside it. The row is never displaced, so `y` needs no pad at all; `x`
    // needs the bend, and `round(d)` can overshoot `|amplitude|` by half a texel.
    //
    // EPS is slack for the **rasteriser's sub-pixel grid**, not for float error: Vulkan
    // guarantees only 8 sub-pixel bits, so a quad edge sitting exactly on a pixel centre
    // can snap 1/256 of a pixel the wrong way and lose that whole row of the sprite.
    // (Measured: at 0.001 texels the real ring world's mean |delta| against the CPU
    // moved 15.996 -> 16.003, which is that row.) A twentieth of a source texel is at
    // least 12 sub-pixel units at every stamp scale this renderer draws, and at most a
    // tenth of a raster pixel, so it can add a fragment and can never add a *texel*:
    // anything outside the box still samples clear and discards.
    //
    // Bilinear (`--gpu-filter bilinear`): a sample up to half a texel outside the box
    // still takes part of the edge texel through the filter, so it keeps the whole
    // texel of support on both axes that this shader always had. That mode is the
    // fidelity comparison and its quad is deliberately unchanged.
    const EPS = 0.05;
    let filtered = u.time.z > 0.5;
    // The bend's own reach, exactly as the fragment computes it: nothing at all when
    // the stamp carries no bend or no length (which is every ground tile and every rain
    // mark), `|amplitude|` when the displacement lands between texels, and half a texel
    // more when it is rounded to a whole one, because `round` can overshoot.
    var reach = 0.0;
    if (s.iBend.x != 0.0 && s.iBend.w > 0.0) {
        reach = abs(s.iBend.x) + select(0.0, 0.5, u.knobs.w < 0.5);
    }
    let padX = reach + select(EPS, 1.0, filtered);
    let padY = select(EPS, 1.0, filtered);
    let lo = vec2(b0.x - pivot.x - padX, b0.y - pivot.y - padY);
    let hi = vec2(b1.x - pivot.x + padX, b1.y - pivot.y + padY);
    // The stamp's own scale (1, or 0.7 for a juvenile) multiplies the world's S.
    let stampScale = u.time.w * max(s.iShade.z, 1e-3);
    let corner = vec2(select(hi.x, lo.x, (vertex & 1) == 0),
                      select(hi.y, lo.y, (vertex & 2) == 0));

    // The anchor lands on a whole raster pixel: pixel art has no sub-pixel positions.
    // Everything else follows from that, including the S x S blocks in the fragment.
    // `--gpu-filter bilinear` keeps the sub-pixel anchor instead, because the point of
    // that mode is to be the CPU presenter's stamp and the CPU does not snap.
    let anchor = select(floor(s.iAnchor) + 0.5, s.iAnchor, u.time.z > 0.5);
    let side = vec2(-s.iHeading.y, s.iHeading.x);
    let px = anchor + stampScale * (corner.x * s.iHeading + corner.y * side);

    var out: Varyings;
    out.vPlace = vec4(anchor, s.iHeading);
    out.vFrames01 = s.iFrames01;
    out.vFrames23 = s.iFrames23;
    out.vSizePivot = s.iSizePivot;
    out.vWeights = s.iWeights;
    out.vBend = s.iBend;
    out.vMask = s.iMask;
    out.vTone = s.iTone;
    out.vShade = s.iShade;
    let position = vec4(px * u.raster.zw * 2.0 - 1.0, 0.0, 1.0);
    // Written for Vulkan's downward clip-space y; WebGPU's points up.
    out.position = vec4(position.x, -position.y, position.zw);
    return out;
}
